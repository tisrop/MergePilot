use crate::error::{CommandError, CommandErrorCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_DIRECTORY_NAME: &str = "logs";
const ACTIVE_LOG_FILE_NAME: &str = "mergebeacon-errors.jsonl";
const DEFAULT_MAX_ACTIVE_BYTES: u64 = 512 * 1024;
const DEFAULT_MAX_ARCHIVES: usize = 3;
pub const DEFAULT_MAX_EXPORTED_RECORDS: usize = 100;
const MAX_COMMAND_BYTES: usize = 64;
const MAX_REQUEST_ID_BYTES: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ErrorLogInput {
    pub command: String,
    pub request_id: String,
    pub code: CommandErrorCode,
    pub retryable: bool,
    pub http_status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorLogRecord {
    pub timestamp_unix_ms: u64,
    pub command: String,
    pub operation: String,
    pub request_id: String,
    pub code: CommandErrorCode,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

#[derive(Debug, Clone)]
struct ErrorLogConfig {
    active_path: PathBuf,
    max_active_bytes: u64,
    max_archives: usize,
}

#[derive(Debug, Clone)]
pub struct ErrorLogStore {
    config: Option<ErrorLogConfig>,
    lock: Arc<Mutex<()>>,
}

impl ErrorLogStore {
    pub fn new(app_data_dir: Option<PathBuf>) -> Self {
        let config = app_data_dir.map(|directory| ErrorLogConfig {
            active_path: directory.join(LOG_DIRECTORY_NAME).join(ACTIVE_LOG_FILE_NAME),
            max_active_bytes: DEFAULT_MAX_ACTIVE_BYTES,
            max_archives: DEFAULT_MAX_ARCHIVES,
        });
        Self { config, lock: Arc::new(Mutex::new(())) }
    }

    #[cfg(test)]
    fn with_limits(directory: PathBuf, max_active_bytes: u64, max_archives: usize) -> Self {
        Self {
            config: Some(ErrorLogConfig {
                active_path: directory.join(ACTIVE_LOG_FILE_NAME),
                max_active_bytes,
                max_archives,
            }),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn record_input(&self, input: ErrorLogInput) -> io::Result<()> {
        let record = build_record(&input.command, &input.request_id, input.code, input.retryable, input.http_status)?;
        self.write_record(&record)
    }

    pub fn record_command_error(&self, command: &str, error: &CommandError) -> io::Result<()> {
        let record = build_record(command, &error.request_id, error.code, error.retryable, error.http_status)?;
        self.write_record(&record)
    }

    pub fn recent_records(&self, limit: usize) -> io::Result<Vec<ErrorLogRecord>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let Some(config) = &self.config else {
            return Ok(Vec::new());
        };
        let _guard = self.lock.lock().map_err(|_| io::Error::other("error log lock poisoned"))?;

        let mut records = Vec::new();
        for archive in (1..=config.max_archives).rev() {
            read_records(&archive_path(&config.active_path, archive), &mut records)?;
        }
        read_records(&config.active_path, &mut records)?;

        if records.len() > limit {
            records.drain(..records.len() - limit);
        }
        Ok(records)
    }

    pub fn formatted_recent_errors(&self, limit: usize) -> io::Result<(String, usize)> {
        let records = self.recent_records(limit)?;
        let mut output = format!(
            "MergeBeacon {} 近期错误日志（脱敏）\n仅包含时间、命令、操作、错误关联标识、错误类别和 HTTP 状态；不包含正文、代码、凭据或远端地址。\n",
            env!("CARGO_PKG_VERSION")
        );
        if records.is_empty() {
            output.push_str("无已记录错误。\n");
        } else {
            for record in &records {
                let encoded = serde_json::to_string(record).map_err(io::Error::other)?;
                output.push_str(&encoded);
                output.push('\n');
            }
        }
        Ok((output, records.len()))
    }

    fn write_record(&self, record: &ErrorLogRecord) -> io::Result<()> {
        eprintln!(
            "{}",
            serde_json::json!({
                "event": "command_error_context",
                "timestamp_unix_ms": record.timestamp_unix_ms,
                "command": record.command,
                "operation": record.operation,
                "request_id": record.request_id,
                "code": record.code,
                "retryable": record.retryable,
                "http_status": record.http_status,
            })
        );
        let Some(config) = &self.config else {
            return Ok(());
        };
        let _guard = self.lock.lock().map_err(|_| io::Error::other("error log lock poisoned"))?;
        let parent = config.active_path.parent().ok_or_else(|| io::Error::other("error log path has no parent"))?;
        fs::create_dir_all(parent)?;
        set_log_directory_permissions(parent)?;

        let mut encoded = serde_json::to_vec(record).map_err(io::Error::other)?;
        encoded.push(b'\n');
        let current_size = fs::metadata(&config.active_path).map(|metadata| metadata.len()).unwrap_or(0);
        if current_size > 0 && current_size.saturating_add(encoded.len() as u64) > config.max_active_bytes {
            sync_if_exists(&config.active_path)?;
            rotate(config)?;
        }

        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&config.active_path)?;
        file.write_all(&encoded)?;
        file.flush()?;
        Ok(())
    }
}

fn build_record(
    command: &str,
    request_id: &str,
    code: CommandErrorCode,
    retryable: bool,
    http_status: Option<u16>,
) -> io::Result<ErrorLogRecord> {
    let (safe_command, operation) = normalize_command(command)?;
    if request_id.is_empty() || request_id.len() > MAX_REQUEST_ID_BYTES || request_id.chars().any(char::is_control) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid request id"));
    }
    if http_status.is_some_and(|status| !(100..=599).contains(&status)) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid HTTP status"));
    }

    Ok(ErrorLogRecord {
        timestamp_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        command: safe_command,
        operation: operation.to_string(),
        request_id: normalized_request_id(request_id),
        code,
        retryable,
        http_status,
    })
}

fn normalize_command(command: &str) -> io::Result<(String, &'static str)> {
    if command.is_empty()
        || command.len() > MAX_COMMAND_BYTES
        || !command.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
            }
        })
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid command"));
    }

    let operation = operation_for_command(command);
    let safe_command = if operation == "system" { hashed_value("unknown", command) } else { command.to_string() };
    Ok((safe_command, operation))
}

fn normalized_request_id(request_id: &str) -> String {
    if is_backend_request_id(request_id) {
        request_id.to_string()
    } else {
        hashed_value("rid", request_id)
    }
}

fn hashed_value(prefix: &str, value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(prefix.len() + 17);
    encoded.push_str(prefix);
    encoded.push('-');
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn operation_for_command(command: &str) -> &'static str {
    match command {
        command if command.starts_with("auth_") => "authentication",
        "repo_list" => "repository",
        command if command.starts_with("review_inbox_") => "inbox",
        command if command.starts_with("pr_") => "pull_request",
        command if command.starts_with("review_") => "review",
        command if command.starts_with("issue_") => "issue",
        command if command.starts_with("ai_") => "ai",
        command if command.starts_with("update_") => "update",
        command if command.starts_with("desktop_notification_") => "notification",
        "support_info" | "copy_support_info" | "copy_recent_error_logs" | "app_version" => "support",
        "platform_capabilities" => "platform",
        _ => "system",
    }
}

fn rotate(config: &ErrorLogConfig) -> io::Result<()> {
    if config.max_archives == 0 {
        match fs::remove_file(&config.active_path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }

    remove_if_exists(&archive_path(&config.active_path, config.max_archives))?;
    for archive in (1..config.max_archives).rev() {
        rename_if_exists(&archive_path(&config.active_path, archive), &archive_path(&config.active_path, archive + 1))?;
    }
    rename_if_exists(&config.active_path, &archive_path(&config.active_path, 1))
}

fn archive_path(active_path: &Path, archive: usize) -> PathBuf {
    let mut name = OsString::from(active_path.as_os_str());
    name.push(format!(".{archive}"));
    PathBuf::from(name)
}

#[cfg(unix)]
fn set_log_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_log_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn sync_if_exists(path: &Path) -> io::Result<()> {
    match OpenOptions::new().read(true).open(path) {
        Ok(file) => file.sync_all(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn rename_if_exists(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn read_records(path: &Path, output: &mut Vec<ErrorLogRecord>) -> io::Result<()> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<ErrorLogRecord>(&line) else {
            continue;
        };
        if operation_for_command(&record.command) == record.operation
            && (record.operation != "system" || is_hashed_unknown_command(&record.command))
            && is_safe_stored_request_id(&record.request_id)
            && record.http_status.is_none_or(|status| (100..=599).contains(&status))
        {
            output.push(record);
        }
    }
    Ok(())
}

fn is_hashed_unknown_command(command: &str) -> bool {
    command.starts_with("unknown-") && command.len() == 24 && command[8..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_safe_stored_request_id(request_id: &str) -> bool {
    is_backend_request_id(request_id)
        || (request_id.starts_with("rid-")
            && request_id.len() == 20
            && request_id[4..].bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn is_backend_request_id(request_id: &str) -> bool {
    request_id.len() == 37
        && request_id.starts_with("err-")
        && request_id.as_bytes().get(20) == Some(&b'-')
        && request_id[4..20].bytes().all(|byte| byte.is_ascii_hexdigit())
        && request_id[21..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{archive_path, ErrorLogInput, ErrorLogStore, ACTIVE_LOG_FILE_NAME};
    use crate::error::{CommandError, CommandErrorCode};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_directory(test_name: &str) -> PathBuf {
        let suffix = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("mergebeacon-error-log-{}-{test_name}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn input(command: &str, request_id: &str) -> ErrorLogInput {
        ErrorLogInput {
            command: command.to_string(),
            request_id: request_id.to_string(),
            code: CommandErrorCode::RateLimited,
            retryable: true,
            http_status: Some(429),
        }
    }

    #[test]
    fn persists_only_safe_metadata_and_hashes_untrusted_request_ids() {
        let directory = temp_directory("safe-record");
        let store = ErrorLogStore::with_limits(directory.clone(), 4096, 1);
        store.record_input(input("pr_detail", "secret-token-https://private.example/repo")).expect("record error");

        let content = fs::read_to_string(directory.join(ACTIVE_LOG_FILE_NAME)).expect("read log");
        assert!(content.contains("\"command\":\"pr_detail\""));
        assert!(content.contains("\"operation\":\"pull_request\""));
        assert!(content.contains("\"code\":\"rate_limited\""));
        assert!(!content.contains("secret-token"));
        assert!(!content.contains("private.example"));
        assert!(!content.contains("message"));

        let records = store.recent_records(10).expect("read recent records");
        assert_eq!(records.len(), 1);
        assert!(records[0].request_id.starts_with("rid-"));
        assert_eq!(records[0].request_id.len(), 20);
        let (exported, count) = store.formatted_recent_errors(10).expect("format recent records");
        assert_eq!(count, 1);
        assert!(exported.contains("\"command\":\"pr_detail\""));
        assert!(!exported.contains("secret-token"));
        assert!(!exported.contains("private.example"));
        assert!(!exported.contains("message"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn preserves_backend_request_ids_for_cross_log_correlation() {
        let directory = temp_directory("backend-request-id");
        let store = ErrorLogStore::with_limits(directory.clone(), 4096, 1);
        let error = CommandError::from("网络请求失败，请稍后重试");
        let expected_request_id = error.request_id.clone();
        store.record_command_error("pr_detail", &error).expect("record command error");

        let records = store.recent_records(10).expect("read recent records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].request_id, expected_request_id);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn records_unmapped_commands_as_hashed_system_entries() {
        let directory = temp_directory("validation");
        let store = ErrorLogStore::with_limits(directory.clone(), 4096, 1);
        store.record_input(input("future_command", "request-1")).expect("record unmapped command");

        let content = fs::read_to_string(directory.join(ACTIVE_LOG_FILE_NAME)).expect("read log");
        assert!(content.contains("\"operation\":\"system\""));
        assert!(content.contains("\"command\":\"unknown-"));
        assert!(!content.contains("future_command"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_malformed_commands_and_invalid_statuses() {
        let directory = temp_directory("validation-reject");
        let store = ErrorLogStore::with_limits(directory.clone(), 4096, 1);
        assert!(store.record_input(input("future-command", "request-1")).is_err());

        let mut invalid_status = input("pr_detail", "request-2");
        invalid_status.http_status = Some(99);
        assert!(store.record_input(invalid_status).is_err());
        assert!(!directory.join(ACTIVE_LOG_FILE_NAME).exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rotates_bounded_files_and_keeps_recent_records() {
        let directory = temp_directory("rotation");
        let store = ErrorLogStore::with_limits(directory.clone(), 220, 2);
        for index in 0..12 {
            store.record_input(input("review_comments_list", &format!("request-{index}"))).expect("record error");
        }

        let active = directory.join(ACTIVE_LOG_FILE_NAME);
        assert!(active.exists());
        assert!(archive_path(&active, 1).exists());
        assert!(archive_path(&active, 2).exists());
        assert!(!archive_path(&active, 3).exists());
        assert!(fs::metadata(&active).expect("active metadata").len() <= 220);
        assert!(fs::metadata(archive_path(&active, 1)).expect("archive 1 metadata").len() <= 220);
        assert!(fs::metadata(archive_path(&active, 2)).expect("archive 2 metadata").len() <= 220);

        let records = store.recent_records(3).expect("read recent records");
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record.command == "review_comments_list"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn ignores_corrupt_and_untrusted_lines_during_export() {
        let directory = temp_directory("corrupt-lines");
        let active = directory.join(ACTIVE_LOG_FILE_NAME);
        fs::write(
            &active,
            concat!(
                "not-json\n",
                "{\"timestamp_unix_ms\":1,\"command\":\"pr_detail\",\"operation\":\"pull_request\",\"request_id\":\"raw-secret\",\"code\":\"unknown\",\"retryable\":false}\n",
                "{\"timestamp_unix_ms\":2,\"command\":\"not_a_command\",\"operation\":\"system\",\"request_id\":\"rid-0123456789abcdef\",\"code\":\"unknown\",\"retryable\":false}\n"
            ),
        )
        .expect("write corrupt log");
        let store = ErrorLogStore::with_limits(directory.clone(), 4096, 1);

        assert!(store.recent_records(10).expect("read records").is_empty());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn disabled_store_is_a_safe_noop() {
        let store = ErrorLogStore::new(None);
        store.record_input(input("pr_detail", "request-1")).expect("disabled write");
        assert!(store.recent_records(10).expect("disabled read").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn restricts_the_log_directory_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temp_directory("directory-permissions");
        let store = ErrorLogStore::new(Some(directory.clone()));
        store.record_input(input("pr_detail", "request-1")).expect("record error");

        let log_directory = directory.join("logs");
        let mode = fs::metadata(log_directory).expect("read log directory metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let _ = fs::remove_dir_all(directory);
    }
}

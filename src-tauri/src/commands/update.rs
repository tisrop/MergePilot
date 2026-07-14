use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::{AppState, UpdateOperationGuard};
use tauri_plugin_updater::{Error as UpdaterError, UpdaterExt};

#[derive(Debug, Serialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub update_mode: UpdateMode,
}

const MAX_RELEASE_NOTES_CHARS: usize = 16_000;
const RELEASE_NOTES_TRUNCATED_SUFFIX: &str = "\n\n[更新说明过长，已截断]";
const WINDOWS_PORTABLE_URL_PATH: [&str; 3] = ["portable", "windows-x86_64", "url"];
const WINDOWS_PORTABLE_SIGNATURE_PATH: [&str; 3] = ["portable", "windows-x86_64", "signature"];
const MAX_UPDATE_SIGNATURE_BYTES: usize = 16 * 1024;
const RELEASE_DOWNLOAD_BASE: &str = "https://github.com/tisrop/MergePilot/releases/download";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    Installer,
    Portable,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateProgressEvent {
    pub request_id: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub phase: &'static str,
}

fn validate_update_request_id(request_id: &str) -> Result<(), String> {
    if request_id.is_empty()
        || request_id.len() > 64
        || !request_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("更新请求标识格式无效".into());
    }
    Ok(())
}

fn validate_expected_version(version: &str) -> Result<(), String> {
    if version.is_empty()
        || version.len() > 64
        || !version.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err("预期更新版本格式无效".into());
    }
    Ok(())
}

fn ensure_expected_update_version(expected: &str, actual: &str) -> Result<(), String> {
    validate_expected_version(expected)?;
    if expected != actual {
        return Err(format!("可用更新已从 v{expected} 变更为 v{actual}，请重新检查并确认后再安装"));
    }
    Ok(())
}

fn update_mode_for(is_windows: bool, has_bundle_type: bool) -> UpdateMode {
    if is_windows && !has_bundle_type {
        UpdateMode::Portable
    } else {
        UpdateMode::Installer
    }
}

fn current_update_mode() -> UpdateMode {
    update_mode_for(cfg!(target_os = "windows"), tauri::utils::platform::bundle_type().is_some())
}

fn ensure_installer_update_mode(mode: UpdateMode) -> Result<(), String> {
    if mode == UpdateMode::Portable {
        return Err("Windows 便携版不能安装 MSI 或 NSIS 更新包，请下载便携版可执行文件更新".into());
    }
    Ok(())
}

fn expected_portable_download_url(version: &str) -> Result<String, String> {
    validate_expected_version(version)?;
    Ok(format!("{RELEASE_DOWNLOAD_BASE}/v{version}/MergePilot_{version}_x64-portable.exe"))
}

fn metadata_string<'a>(release: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(release, |value, key| value.get(*key)).and_then(serde_json::Value::as_str)
}

fn portable_download_details(release: &serde_json::Value, version: &str) -> Result<(String, String), String> {
    let expected = expected_portable_download_url(version)?;
    let actual = metadata_string(release, &WINDOWS_PORTABLE_URL_PATH)
        .ok_or_else(|| "更新元数据缺少 Windows 便携版可执行文件，请联系发布者修复".to_string())?;

    if actual != expected {
        return Err("更新元数据中的 Windows 便携版可执行文件地址无效，已拒绝下载".into());
    }

    let signature = metadata_string(release, &WINDOWS_PORTABLE_SIGNATURE_PATH)
        .ok_or_else(|| "更新元数据缺少 Windows 便携版签名，已拒绝下载".to_string())?;
    if signature.trim().is_empty() || signature.len() > MAX_UPDATE_SIGNATURE_BYTES {
        return Err("更新元数据中的 Windows 便携版签名无效，已拒绝下载".into());
    }

    Ok((actual.to_string(), signature.to_string()))
}

fn sanitize_release_notes(notes: Option<String>) -> Option<String> {
    notes.map(|notes| {
        if notes.chars().count() <= MAX_RELEASE_NOTES_CHARS {
            return notes;
        }
        let mut truncated: String = notes.chars().take(MAX_RELEASE_NOTES_CHARS).collect();
        truncated.push_str(RELEASE_NOTES_TRUNCATED_SUFFIX);
        truncated
    })
}

fn check_result(
    current_version: String,
    update_mode: UpdateMode,
    update: Option<(String, Option<String>, Option<String>)>,
) -> UpdateCheckResult {
    match update {
        Some((version, notes, published_at)) => UpdateCheckResult {
            current_version,
            available: true,
            version: Some(version),
            notes: sanitize_release_notes(notes),
            published_at,
            update_mode,
        },
        None => UpdateCheckResult {
            current_version,
            available: false,
            version: None,
            notes: None,
            published_at: None,
            update_mode,
        },
    }
}

fn update_error(error: UpdaterError) -> String {
    match error {
        UpdaterError::ReleaseNotFound => {
            "更新源暂未提供有效的发布元数据，请确认已发布包含 latest.json 的正式版本后重试".into()
        }
        UpdaterError::TargetNotFound(_) | UpdaterError::TargetsNotFound(_) => {
            "更新元数据缺少当前平台的安装包，请联系发布者修复".into()
        }
        UpdaterError::Serialization(_)
        | UpdaterError::Semver(_)
        | UpdaterError::UrlParse(_)
        | UpdaterError::Base64(_)
        | UpdaterError::SignatureUtf8(_)
        | UpdaterError::Minisign(_) => "更新元数据或签名格式无效，已拒绝更新".into(),
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => "检查更新失败，请检查网络后重试".into(),
        UpdaterError::InsecureTransportProtocol => "更新源不是安全的 HTTPS 地址，已拒绝连接".into(),
        UpdaterError::UnsupportedArch | UpdaterError::UnsupportedOs => "当前系统或架构暂不支持自动更新".into(),
        UpdaterError::EmptyEndpoints => "应用未配置更新源，无法检查更新".into(),
        _ => "检查更新失败，请稍后重试".into(),
    }
}

fn download_error(error: UpdaterError) -> String {
    match error {
        UpdaterError::Base64(_) | UpdaterError::SignatureUtf8(_) | UpdaterError::Minisign(_) => {
            "更新包签名验证失败，已停止安装".into()
        }
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => "下载更新失败，请检查网络后重试".into(),
        UpdaterError::InsecureTransportProtocol => "更新包地址不是安全的 HTTPS 地址，已拒绝下载".into(),
        _ => "下载更新失败，请稍后重试".into(),
    }
}

fn install_error(_error: UpdaterError) -> String {
    "安装更新失败，应用未重启，请稍后重试".into()
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateCheckResult, String> {
    let updater = app.updater().map_err(|_| "初始化更新检查失败，请稍后重试".to_string())?;
    let update = updater.check().await.map_err(update_error)?;
    let update_mode = current_update_mode();
    if let Some(update) = update.as_ref() {
        if update_mode == UpdateMode::Portable {
            portable_download_details(&update.raw_json, &update.version)?;
        }
    }
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    Ok(check_result(
        current_version,
        update_mode,
        update.map(|update| (update.version, update.body, update.date.map(|date| date.to_string()))),
    ))
}

#[cfg(any(windows, test))]
const PORTABLE_UPDATE_HELPER_SCRIPT: &str = r#"param(
  [Parameter(Mandatory = $true)][long]$ParentProcessId,
  [Parameter(Mandatory = $true)][string]$CurrentExe,
  [Parameter(Mandatory = $true)][string]$StagedExe,
  [Parameter(Mandatory = $true)][string]$BackupExe,
  [Parameter(Mandatory = $true)][string]$LogFile
)

$ErrorActionPreference = "Stop"
$parentExited = $false
$newVersionRunning = $false
$oldVersionRelaunched = $false

function Write-UpdateLog([string]$Message) {
  try {
    [System.IO.File]::WriteAllText($LogFile, $Message, [System.Text.Encoding]::UTF8)
  } catch {}
}

try {
  $deadline = (Get-Date).AddSeconds(60)
  while ($null -ne (Get-Process -Id $ParentProcessId -ErrorAction SilentlyContinue)) {
    if ((Get-Date) -ge $deadline) {
      throw "Timed out waiting for MergePilot to exit"
    }
    Start-Sleep -Milliseconds 100
  }
  $parentExited = $true
  Start-Sleep -Milliseconds 250

  Move-Item -LiteralPath $CurrentExe -Destination $BackupExe
  try {
    Move-Item -LiteralPath $StagedExe -Destination $CurrentExe
  } catch {
    Move-Item -LiteralPath $BackupExe -Destination $CurrentExe
    throw
  }

  $newProcess = $null
  try {
    $newProcess = Start-Process -FilePath $CurrentExe -PassThru
    Start-Sleep -Seconds 3
    $newProcess.Refresh()
    if ($newProcess.HasExited) {
      throw "New MergePilot exited during startup"
    }
    $newVersionRunning = $true
  } catch {
    if ($null -ne $newProcess -and -not $newProcess.HasExited) {
      Stop-Process -Id $newProcess.Id -Force -ErrorAction SilentlyContinue
      $newProcess.WaitForExit(5000) | Out-Null
    }
    if (Test-Path -LiteralPath $CurrentExe) {
      Remove-Item -LiteralPath $CurrentExe -Force
    }
    if (Test-Path -LiteralPath $BackupExe) {
      Move-Item -LiteralPath $BackupExe -Destination $CurrentExe
    }
    if (Test-Path -LiteralPath $CurrentExe) {
      Start-Process -FilePath $CurrentExe
      $oldVersionRelaunched = $true
    }
    throw
  }

  try {
    Remove-Item -LiteralPath $BackupExe -Force
    Remove-Item -LiteralPath $LogFile -Force -ErrorAction SilentlyContinue
  } catch {
    Write-UpdateLog ("The new version is running, but cleanup failed.`r`n" + ($_ | Out-String))
  }
} catch {
  Write-UpdateLog ($_ | Out-String)
  if ($parentExited -and -not $newVersionRunning -and -not $oldVersionRelaunched) {
    try {
      if (-not (Test-Path -LiteralPath $CurrentExe) -and (Test-Path -LiteralPath $BackupExe)) {
        Move-Item -LiteralPath $BackupExe -Destination $CurrentExe
      }
      if (Test-Path -LiteralPath $CurrentExe) {
        Start-Process -FilePath $CurrentExe
        $oldVersionRelaunched = $true
      }
    } catch {
      Write-UpdateLog ((Get-Content -LiteralPath $LogFile -Raw -ErrorAction SilentlyContinue) + "`r`nRollback or relaunch failed:`r`n" + ($_ | Out-String))
    }
  }
} finally {
  Remove-Item -LiteralPath $StagedExe -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
}
"#;

#[cfg(any(windows, test))]
fn portable_update_file_names(request_id: &str) -> Result<(String, String, String, String), String> {
    validate_update_request_id(request_id)?;
    Ok((
        format!(".mergepilot-update-{request_id}.exe"),
        format!(".mergepilot-backup-{request_id}.exe"),
        format!("mergepilot-portable-update-{request_id}.ps1"),
        format!("mergepilot-portable-update-{request_id}.log"),
    ))
}

#[cfg(windows)]
fn stage_portable_replacement(request_id: &str, bytes: &[u8]) -> Result<(), String> {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let current_exe = std::env::current_exe().map_err(|_| "无法确定当前便携版 EXE 位置，已停止更新".to_string())?;
    let parent = current_exe.parent().ok_or_else(|| "当前便携版 EXE 路径无效，已停止更新".to_string())?;
    let staged_exe = parent.join(format!(".mergepilot-update-{request_id}.exe"));
    let backup_exe = parent.join(format!(".mergepilot-backup-{request_id}.exe"));
    let temp_dir = std::env::temp_dir();
    let helper_script = temp_dir.join(format!("mergepilot-portable-update-{request_id}.ps1"));
    let log_file = temp_dir.join(format!("mergepilot-portable-update-{request_id}.log"));

    let mut staged = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged_exe)
        .map_err(|_| "当前 EXE 所在目录不可写，无法自动替换；请将便携版移到可写目录后重试".to_string())?;
    if staged.write_all(bytes).and_then(|()| staged.sync_all()).is_err() {
        let _ = fs::remove_file(&staged_exe);
        return Err("写入新版便携 EXE 失败，旧版本未受影响".into());
    }
    drop(staged);

    let write_helper = || -> std::io::Result<()> {
        let mut helper = OpenOptions::new().write(true).create_new(true).open(&helper_script)?;
        helper.write_all(PORTABLE_UPDATE_HELPER_SCRIPT.as_bytes())?;
        helper.sync_all()
    };
    if write_helper().is_err() {
        let _ = fs::remove_file(&staged_exe);
        let _ = fs::remove_file(&helper_script);
        return Err("创建便携版更新助手失败，旧版本未受影响".into());
    }

    let spawn_result = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&helper_script)
        .arg("-ParentProcessId")
        .arg(std::process::id().to_string())
        .arg("-CurrentExe")
        .arg(&current_exe)
        .arg("-StagedExe")
        .arg(&staged_exe)
        .arg("-BackupExe")
        .arg(&backup_exe)
        .arg("-LogFile")
        .arg(&log_file)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();

    if spawn_result.is_err() {
        let _ = fs::remove_file(&staged_exe);
        let _ = fs::remove_file(&helper_script);
        return Err("启动便携版更新助手失败，旧版本未受影响".into());
    }

    Ok(())
}

#[cfg(not(windows))]
fn stage_portable_replacement(_request_id: &str, _bytes: &[u8]) -> Result<(), String> {
    Err("当前系统不支持 Windows 便携版自动替换".into())
}

fn schedule_portable_exit(app: AppHandle, operation: UpdateOperationGuard) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        app.exit(0);
        drop(operation);
    });
}

#[tauri::command]
pub async fn update_download_and_replace_portable(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    expected_version: String,
) -> Result<(), String> {
    validate_update_request_id(&request_id)?;
    if current_update_mode() != UpdateMode::Portable {
        return Err("当前版本不是 Windows 便携版，不能使用便携版自动替换流程".into());
    }
    let operation = state.operations.begin_update().await?;

    let updater = app.updater().map_err(|_| "初始化更新下载失败，请稍后重试".to_string())?;
    let mut update =
        updater.check().await.map_err(update_error)?.ok_or_else(|| "当前已是最新版本，无需更新".to_string())?;
    ensure_expected_update_version(&expected_version, &update.version)?;
    let (portable_url, portable_signature) = portable_download_details(&update.raw_json, &update.version)?;
    update.download_url = portable_url.parse().map_err(|_| "Windows 便携版下载地址无效，已拒绝下载".to_string())?;
    update.signature = portable_signature;

    let progress_app = app.clone();
    let progress_request_id = request_id.clone();
    let finish_app = app.clone();
    let finish_request_id = request_id.clone();
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            move |chunk_size, total| {
                downloaded = downloaded.saturating_add(chunk_size as u64);
                let _ = progress_app.emit(
                    "update-progress",
                    UpdateProgressEvent {
                        request_id: progress_request_id.clone(),
                        downloaded,
                        total,
                        phase: "downloading",
                    },
                );
            },
            move || {
                let _ = finish_app.emit(
                    "update-progress",
                    UpdateProgressEvent {
                        request_id: finish_request_id,
                        downloaded: 0,
                        total: None,
                        phase: "installing",
                    },
                );
            },
        )
        .await
        .map_err(download_error)?;

    stage_portable_replacement(&request_id, &bytes)?;
    schedule_portable_exit(app, operation);
    Ok(())
}

#[tauri::command]
pub async fn update_download_and_install(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    expected_version: String,
) -> Result<(), String> {
    validate_update_request_id(&request_id)?;
    ensure_installer_update_mode(current_update_mode())?;
    let _operation = state.operations.begin_update().await?;

    let updater = app.updater().map_err(|_| "初始化更新下载失败，请稍后重试".to_string())?;
    let update =
        updater.check().await.map_err(update_error)?.ok_or_else(|| "当前已是最新版本，无需下载安装".to_string())?;
    ensure_expected_update_version(&expected_version, &update.version)?;

    let progress_app = app.clone();
    let progress_request_id = request_id.clone();
    let finish_app = app.clone();
    let finish_request_id = request_id.clone();
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            move |chunk_size, total| {
                downloaded = downloaded.saturating_add(chunk_size as u64);
                let _ = progress_app.emit(
                    "update-progress",
                    UpdateProgressEvent {
                        request_id: progress_request_id.clone(),
                        downloaded,
                        total,
                        phase: "downloading",
                    },
                );
            },
            move || {
                let _ = finish_app.emit(
                    "update-progress",
                    UpdateProgressEvent {
                        request_id: finish_request_id,
                        downloaded: 0,
                        total: None,
                        phase: "installing",
                    },
                );
            },
        )
        .await
        .map_err(download_error)?;

    update.install(bytes).map_err(install_error)?;
    Ok(())
}

#[tauri::command]
pub async fn update_restart(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let _operation = state.operations.begin_update().await?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::{
        check_result, download_error, ensure_expected_update_version, ensure_installer_update_mode, install_error,
        portable_download_details, portable_update_file_names, sanitize_release_notes, update_error, update_mode_for,
        validate_expected_version, validate_update_request_id, UpdateMode, MAX_RELEASE_NOTES_CHARS,
        MAX_UPDATE_SIGNATURE_BYTES, PORTABLE_UPDATE_HELPER_SCRIPT, RELEASE_NOTES_TRUNCATED_SUFFIX,
    };
    use serde_json::json;
    use tauri_plugin_updater::Error as UpdaterError;

    #[test]
    fn reports_up_to_date_without_remote_fields() {
        let result = check_result("0.3.0".into(), UpdateMode::Installer, None);
        assert!(!result.available);
        assert!(result.version.is_none());
        assert!(result.notes.is_none());
        assert_eq!(result.update_mode, UpdateMode::Installer);
    }

    #[test]
    fn preserves_available_update_metadata_as_untrusted_text() {
        let result = check_result(
            "0.3.0".into(),
            UpdateMode::Portable,
            Some(("0.4.0".into(), Some("<script>不可信说明</script>".into()), Some("2026-07-13".into()))),
        );
        assert!(result.available);
        assert_eq!(result.version.as_deref(), Some("0.4.0"));
        assert_eq!(result.notes.as_deref(), Some("<script>不可信说明</script>"));
        assert_eq!(result.update_mode, UpdateMode::Portable);
    }
    #[test]
    fn explains_missing_or_invalid_release_metadata() {
        assert_eq!(
            update_error(UpdaterError::ReleaseNotFound),
            "更新源暂未提供有效的发布元数据，请确认已发布包含 latest.json 的正式版本后重试"
        );
    }
    #[test]
    fn localizes_updater_failures_without_exposing_remote_values() {
        assert_eq!(
            update_error(UpdaterError::TargetNotFound("secret-target".into())),
            "更新元数据缺少当前平台的安装包，请联系发布者修复"
        );
        assert_eq!(
            update_error(UpdaterError::SignatureUtf8("secret-signature".into())),
            "更新元数据或签名格式无效，已拒绝更新"
        );
        assert_eq!(
            download_error(UpdaterError::Network("https://example.invalid/?token=secret".into())),
            "下载更新失败，请检查网络后重试"
        );
        assert_eq!(
            download_error(UpdaterError::SignatureUtf8("secret-signature".into())),
            "更新包签名验证失败，已停止安装"
        );
        assert_eq!(install_error(UpdaterError::PackageInstallFailed), "安装更新失败，应用未重启，请稍后重试");
    }

    #[test]
    fn truncates_oversized_release_notes_on_utf8_character_boundaries() {
        let notes = "更".repeat(MAX_RELEASE_NOTES_CHARS + 1);
        let sanitized = sanitize_release_notes(Some(notes)).expect("notes should remain present");
        assert!(sanitized.ends_with(RELEASE_NOTES_TRUNCATED_SUFFIX));
        assert_eq!(sanitized.trim_end_matches(RELEASE_NOTES_TRUNCATED_SUFFIX).chars().count(), MAX_RELEASE_NOTES_CHARS);
        assert_eq!(sanitize_release_notes(None), None);
    }

    #[test]
    fn rejects_oversized_or_unsafe_update_inputs() {
        assert!(validate_update_request_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert_eq!(validate_update_request_id("bad\nrequest"), Err("更新请求标识格式无效".into()));
        assert_eq!(validate_update_request_id(&"a".repeat(65)), Err("更新请求标识格式无效".into()));

        assert!(validate_expected_version("1.2.3-beta.1+build.2").is_ok());
        assert_eq!(validate_expected_version("1.2.3\n伪造错误"), Err("预期更新版本格式无效".into()));
        assert_eq!(validate_expected_version(&"1".repeat(65)), Err("预期更新版本格式无效".into()));
    }

    #[test]
    fn requires_reconfirmation_when_available_version_changes() {
        assert!(ensure_expected_update_version("0.4.0", "0.4.0").is_ok());
        assert_eq!(ensure_expected_update_version("", "0.4.0"), Err("预期更新版本格式无效".into()));
        assert_eq!(
            ensure_expected_update_version("0.4.0", "0.5.0"),
            Err("可用更新已从 v0.4.0 变更为 v0.5.0，请重新检查并确认后再安装".into())
        );
    }

    #[test]
    fn selects_portable_mode_only_for_unbundled_windows_executables() {
        assert_eq!(update_mode_for(true, false), UpdateMode::Portable);
        assert_eq!(update_mode_for(true, true), UpdateMode::Installer);
        assert_eq!(update_mode_for(false, false), UpdateMode::Installer);
    }

    #[test]
    fn portable_mode_rejects_installer_updates() {
        assert_eq!(
            ensure_installer_update_mode(UpdateMode::Portable),
            Err("Windows 便携版不能安装 MSI 或 NSIS 更新包，请下载便携版可执行文件更新".into())
        );
        assert!(ensure_installer_update_mode(UpdateMode::Installer).is_ok());
    }

    #[test]
    fn accepts_only_the_versioned_official_signed_portable_executable() {
        let release = json!({
            "portable": {
                "windows-x86_64": {
                    "url": "https://github.com/tisrop/MergePilot/releases/download/v0.4.0/MergePilot_0.4.0_x64-portable.exe",
                    "signature": "trusted-portable-signature"
                }
            }
        });
        assert_eq!(
            portable_download_details(&release, "0.4.0"),
            Ok((
                "https://github.com/tisrop/MergePilot/releases/download/v0.4.0/MergePilot_0.4.0_x64-portable.exe"
                    .into(),
                "trusted-portable-signature".into(),
            ))
        );

        let installer = json!({
            "portable": {
                "windows-x86_64": {
                    "url": "https://github.com/tisrop/MergePilot/releases/download/v0.4.0/Merge.Pilot_0.4.0_x64_en-US.msi",
                    "signature": "signature"
                }
            }
        });
        assert_eq!(
            portable_download_details(&installer, "0.4.0"),
            Err("更新元数据中的 Windows 便携版可执行文件地址无效，已拒绝下载".into())
        );
        let archive = json!({
            "portable": {
                "windows-x86_64": {
                    "url": "https://github.com/tisrop/MergePilot/releases/download/v0.4.0/MergePilot_0.4.0_x64-portable.zip",
                    "signature": "signature"
                }
            }
        });
        assert_eq!(
            portable_download_details(&archive, "0.4.0"),
            Err("更新元数据中的 Windows 便携版可执行文件地址无效，已拒绝下载".into())
        );
        assert_eq!(
            portable_download_details(&json!({}), "0.4.0"),
            Err("更新元数据缺少 Windows 便携版可执行文件，请联系发布者修复".into())
        );
    }

    #[test]
    fn rejects_missing_empty_or_oversized_portable_signatures() {
        let url = "https://github.com/tisrop/MergePilot/releases/download/v0.4.0/MergePilot_0.4.0_x64-portable.exe";
        let missing = json!({ "portable": { "windows-x86_64": { "url": url } } });
        assert_eq!(
            portable_download_details(&missing, "0.4.0"),
            Err("更新元数据缺少 Windows 便携版签名，已拒绝下载".into())
        );

        for signature in ["   ".to_string(), "s".repeat(MAX_UPDATE_SIGNATURE_BYTES + 1)] {
            let invalid = json!({
                "portable": { "windows-x86_64": { "url": url, "signature": signature } }
            });
            assert_eq!(
                portable_download_details(&invalid, "0.4.0"),
                Err("更新元数据中的 Windows 便携版签名无效，已拒绝下载".into())
            );
        }
    }

    #[test]
    fn portable_helper_names_require_a_validated_request_id() {
        assert_eq!(
            portable_update_file_names("safe-request-1"),
            Ok((
                ".mergepilot-update-safe-request-1.exe".into(),
                ".mergepilot-backup-safe-request-1.exe".into(),
                "mergepilot-portable-update-safe-request-1.ps1".into(),
                "mergepilot-portable-update-safe-request-1.log".into(),
            ))
        );
        assert_eq!(portable_update_file_names("../unsafe"), Err("更新请求标识格式无效".into()));
    }

    #[test]
    fn portable_helper_waits_replaces_rolls_back_and_relaunches() {
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("Get-Process -Id $ParentProcessId"));
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("Move-Item -LiteralPath $CurrentExe -Destination $BackupExe"));
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("Move-Item -LiteralPath $StagedExe -Destination $CurrentExe"));
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("Move-Item -LiteralPath $BackupExe -Destination $CurrentExe"));
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("Start-Process -FilePath $CurrentExe"));
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("New MergePilot exited during startup"));
        assert!(PORTABLE_UPDATE_HELPER_SCRIPT.contains("Remove-Item -LiteralPath $PSCommandPath"));
    }
}

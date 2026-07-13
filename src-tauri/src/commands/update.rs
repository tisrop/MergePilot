use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;
use tauri_plugin_updater::{Error as UpdaterError, UpdaterExt};

#[derive(Debug, Serialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

const MAX_RELEASE_NOTES_CHARS: usize = 16_000;
const RELEASE_NOTES_TRUNCATED_SUFFIX: &str = "\n\n[更新说明过长，已截断]";

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
    update: Option<(String, Option<String>, Option<String>)>,
) -> UpdateCheckResult {
    match update {
        Some((version, notes, published_at)) => UpdateCheckResult {
            current_version,
            available: true,
            version: Some(version),
            notes: sanitize_release_notes(notes),
            published_at,
        },
        None => UpdateCheckResult { current_version, available: false, version: None, notes: None, published_at: None },
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
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    Ok(check_result(
        current_version,
        update.map(|update| (update.version, update.body, update.date.map(|date| date.to_string()))),
    ))
}

#[tauri::command]
pub async fn update_download_and_install(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    expected_version: String,
) -> Result<(), String> {
    validate_update_request_id(&request_id)?;
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
        check_result, download_error, ensure_expected_update_version, install_error, sanitize_release_notes,
        update_error, validate_expected_version, validate_update_request_id, MAX_RELEASE_NOTES_CHARS,
        RELEASE_NOTES_TRUNCATED_SUFFIX,
    };
    use tauri_plugin_updater::Error as UpdaterError;

    #[test]
    fn reports_up_to_date_without_remote_fields() {
        let result = check_result("0.3.0".into(), None);
        assert!(!result.available);
        assert!(result.version.is_none());
        assert!(result.notes.is_none());
    }

    #[test]
    fn preserves_available_update_metadata_as_untrusted_text() {
        let result = check_result(
            "0.3.0".into(),
            Some(("0.4.0".into(), Some("<script>不可信说明</script>".into()), Some("2026-07-13".into()))),
        );
        assert!(result.available);
        assert_eq!(result.version.as_deref(), Some("0.4.0"));
        assert_eq!(result.notes.as_deref(), Some("<script>不可信说明</script>"));
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
}

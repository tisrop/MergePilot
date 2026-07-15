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
    pub update_mode: UpdateMode,
    pub portable_download_url: Option<String>,
}

const MAX_RELEASE_NOTES_CHARS: usize = 16_000;
const RELEASE_NOTES_TRUNCATED_SUFFIX: &str = "\n\n[更新说明过长，已截断]";
const WINDOWS_PORTABLE_URL_PATH: [&str; 3] = ["portable", "windows-x86_64", "url"];
const RELEASE_DOWNLOAD_BASE: &str = "https://github.com/tisrop/MergeBeacon/releases/download";
type AvailableUpdate = (String, Option<String>, Option<String>, Option<String>);

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
        return Err("Windows 便携版不支持应用内安装，请下载 ZIP 后手动解压覆盖".into());
    }
    Ok(())
}

fn expected_portable_download_url(version: &str) -> Result<String, String> {
    validate_expected_version(version)?;
    Ok(format!("{RELEASE_DOWNLOAD_BASE}/v{version}/MergeBeacon_{version}_x64-portable.zip"))
}

fn metadata_string<'a>(release: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(release, |value, key| value.get(*key)).and_then(serde_json::Value::as_str)
}

fn portable_download_url(release: &serde_json::Value, version: &str) -> Result<String, String> {
    let expected = expected_portable_download_url(version)?;
    let actual = metadata_string(release, &WINDOWS_PORTABLE_URL_PATH)
        .ok_or_else(|| "更新元数据缺少 Windows 便携版 ZIP，请联系发布者修复".to_string())?;

    if actual != expected {
        return Err("更新元数据中的 Windows 便携版 ZIP 地址无效，已拒绝打开".into());
    }

    Ok(actual.to_string())
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
    update: Option<AvailableUpdate>,
) -> UpdateCheckResult {
    match update {
        Some((version, notes, published_at, portable_download_url)) => UpdateCheckResult {
            current_version,
            available: true,
            version: Some(version),
            notes: sanitize_release_notes(notes),
            published_at,
            update_mode,
            portable_download_url,
        },
        None => UpdateCheckResult {
            current_version,
            available: false,
            version: None,
            notes: None,
            published_at: None,
            update_mode,
            portable_download_url: None,
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
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let update = update
        .map(|update| {
            let portable_download_url = if update_mode == UpdateMode::Portable {
                Some(portable_download_url(&update.raw_json, &update.version)?)
            } else {
                None
            };
            Ok::<_, String>((
                update.version,
                update.body,
                update.date.map(|date| date.to_string()),
                portable_download_url,
            ))
        })
        .transpose()?;
    Ok(check_result(current_version, update_mode, update))
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
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_update_metadata_and_portable_url() {
        let url = "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64-portable.zip";
        let result = check_result(
            "0.3.0".into(),
            UpdateMode::Portable,
            Some(("0.4.0".into(), Some("更新说明".into()), None, Some(url.into()))),
        );
        assert!(result.available);
        assert_eq!(result.portable_download_url.as_deref(), Some(url));
        assert!(check_result("0.4.0".into(), UpdateMode::Installer, None).portable_download_url.is_none());
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
            Err("Windows 便携版不支持应用内安装，请下载 ZIP 后手动解压覆盖".into())
        );
        assert!(ensure_installer_update_mode(UpdateMode::Installer).is_ok());
    }

    #[test]
    fn accepts_only_official_versioned_portable_zip_url() {
        let expected =
            "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64-portable.zip";
        let release = json!({"portable":{"windows-x86_64":{"url":expected}}});
        assert_eq!(portable_download_url(&release, "0.4.0"), Ok(expected.into()));
        for invalid in [
            "https://example.com/MergeBeacon_0.4.0_x64-portable.zip",
            "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.1/MergeBeacon_0.4.1_x64-portable.zip",
            "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64-portable.exe",
            "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64_en-US.msi",
        ] {
            let release = json!({"portable":{"windows-x86_64":{"url":invalid}}});
            assert!(portable_download_url(&release, "0.4.0").is_err());
        }
        assert!(portable_download_url(&json!({}), "0.4.0").is_err());
    }

    #[test]
    fn validates_inputs_and_version_confirmation() {
        assert!(validate_update_request_id("safe-request-1").is_ok());
        assert!(validate_update_request_id("../unsafe").is_err());
        assert!(validate_expected_version("1.2.3-beta.1+build.2").is_ok());
        assert!(validate_expected_version("1.2.3\nunsafe").is_err());
        assert!(ensure_expected_update_version("0.4.0", "0.4.0").is_ok());
        assert!(ensure_expected_update_version("0.4.0", "0.4.1").is_err());
    }

    #[test]
    fn truncates_oversized_release_notes_on_utf8_boundaries() {
        let sanitized = sanitize_release_notes(Some("更".repeat(MAX_RELEASE_NOTES_CHARS + 1))).unwrap();
        assert!(sanitized.ends_with(RELEASE_NOTES_TRUNCATED_SUFFIX));
        assert_eq!(sanitized.trim_end_matches(RELEASE_NOTES_TRUNCATED_SUFFIX).chars().count(), MAX_RELEASE_NOTES_CHARS);
    }

    #[test]
    fn localizes_updater_errors() {
        assert_eq!(
            update_error(UpdaterError::ReleaseNotFound),
            "更新源暂未提供有效的发布元数据，请确认已发布包含 latest.json 的正式版本后重试"
        );
        assert_eq!(download_error(UpdaterError::Network("hidden".into())), "下载更新失败，请检查网络后重试");
        assert_eq!(install_error(UpdaterError::PackageInstallFailed), "安装更新失败，应用未重启，请稍后重试");
    }
}

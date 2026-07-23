use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::error::{CommandError, CommandResult};
use crate::error_log::{ErrorLogStore, DEFAULT_MAX_EXPORTED_RECORDS};
use crate::local_store::CommentSnapshotStore;
use crate::state::AppState;
use crate::vault::CredentialStorage;

const SUPPORTED_PLATFORMS: [&str; 3] = ["github", "gitlab", "gitee"];

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SupportInfo {
    app_version: String,
    operating_system: String,
    architecture: String,
    current_platform: String,
    platform_endpoint: String,
    credential_storage: String,
    ai_configured: bool,
    ai_endpoint: String,
    local_cache_available: bool,
    formatted: String,
}

struct SupportInfoInput<'a> {
    platform: &'a str,
    custom_url: Option<&'a str>,
    credential_storage: Option<CredentialStorage>,
    ai_configured: bool,
    ai_endpoint: &'a str,
    local_cache_available: bool,
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "gitee" => "Gitee",
        _ => "未知平台",
    }
}

fn credential_storage_label(storage: Option<CredentialStorage>) -> &'static str {
    match storage {
        Some(CredentialStorage::SystemKeyring) => "系统 Keyring",
        Some(CredentialStorage::EncryptedFile) => "加密文件降级",
        None => "未配置",
    }
}

fn sanitized_ai_endpoint(endpoint: &str) -> &'static str {
    let normalized = endpoint.trim().trim_end_matches('/').to_ascii_lowercase();
    if normalized == "https://api.openai.com" || normalized.starts_with("https://api.openai.com/") {
        "OpenAI 官方服务"
    } else {
        "自托管（地址已隐藏）"
    }
}

fn build_support_info(input: SupportInfoInput<'_>) -> SupportInfo {
    let current_platform = platform_label(input.platform).to_string();
    let platform_endpoint = if input.custom_url.is_some() {
        "自托管（地址已隐藏）".to_string()
    } else {
        "官方服务".to_string()
    };
    let credential_storage = credential_storage_label(input.credential_storage).to_string();
    let ai_endpoint = sanitized_ai_endpoint(input.ai_endpoint).to_string();
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let operating_system = std::env::consts::OS.to_string();
    let architecture = std::env::consts::ARCH.to_string();
    let ai_status = if input.ai_configured { "已配置" } else { "未配置" };
    let cache_status = if input.local_cache_available { "可用" } else { "不可用" };
    let formatted = format!(
        "MergeBeacon {app_version}\n操作系统：{operating_system}\n架构：{architecture}\n当前平台：{current_platform}\n平台服务：{platform_endpoint}\nToken 存储：{credential_storage}\nAI 配置：{ai_status}\nAI 服务：{ai_endpoint}\n本地评论缓存：{cache_status}"
    );

    SupportInfo {
        app_version,
        operating_system,
        architecture,
        current_platform,
        platform_endpoint,
        credential_storage,
        ai_configured: input.ai_configured,
        ai_endpoint,
        local_cache_available: input.local_cache_available,
        formatted,
    }
}

#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
pub async fn support_info(
    state: State<'_, AppState>,
    comment_store: State<'_, CommentSnapshotStore>,
    platform: String,
) -> CommandResult<SupportInfo> {
    if !SUPPORTED_PLATFORMS.contains(&platform.as_str()) {
        return Err("不支持的平台，无法生成诊断信息".into());
    }

    let custom_url = state.token_vault.get_custom_url(&platform);
    let credential_storage = state.token_vault.credential_storage(&platform).map_err(CommandError::from)?;
    let ai_config = state.ai_config.get_config().map_err(CommandError::from)?;

    Ok(build_support_info(SupportInfoInput {
        platform: &platform,
        custom_url: custom_url.as_deref(),
        credential_storage,
        ai_configured: ai_config.api_key_configured,
        ai_endpoint: &ai_config.endpoint,
        local_cache_available: comment_store.is_available(),
    }))
}

#[tauri::command]
pub async fn copy_support_info(
    app: AppHandle,
    state: State<'_, AppState>,
    comment_store: State<'_, CommentSnapshotStore>,
    platform: String,
) -> CommandResult<()> {
    let info = support_info(state, comment_store, platform).await?;
    app.clipboard()
        .write_text(info.formatted)
        .map_err(|error| CommandError::from(format!("写入系统剪贴板失败：{error}")))
}

#[tauri::command]
pub fn copy_recent_error_logs(app: AppHandle, error_logs: State<'_, ErrorLogStore>) -> CommandResult<usize> {
    let (formatted, count) = error_logs
        .formatted_recent_errors(DEFAULT_MAX_EXPORTED_RECORDS)
        .map_err(|_| CommandError::from("近期错误日志暂不可用"))?;
    app.clipboard()
        .write_text(formatted)
        .map_err(|error| CommandError::from(format!("写入系统剪贴板失败：{error}")))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::{build_support_info, sanitized_ai_endpoint, SupportInfoInput};
    use crate::vault::CredentialStorage;

    #[test]
    fn hides_self_hosted_platform_and_ai_addresses() {
        let platform_url = "https://user:secret@git.example.internal/proxy/api?token=hidden";
        let ai_url = "https://key@ai.example.internal/v1?api_key=hidden";
        let info = build_support_info(SupportInfoInput {
            platform: "gitlab",
            custom_url: Some(platform_url),
            credential_storage: Some(CredentialStorage::EncryptedFile),
            ai_configured: true,
            ai_endpoint: ai_url,
            local_cache_available: true,
        });

        assert_eq!(info.platform_endpoint, "自托管（地址已隐藏）");
        assert_eq!(info.ai_endpoint, "自托管（地址已隐藏）");
        assert!(!info.formatted.contains(platform_url));
        assert!(!info.formatted.contains(ai_url));
        assert!(!info.formatted.contains("secret"));
        assert!(!info.formatted.contains("api_key"));
    }

    #[test]
    fn identifies_only_the_official_openai_endpoint() {
        assert_eq!(sanitized_ai_endpoint("https://api.openai.com/v1"), "OpenAI 官方服务");
        assert_eq!(sanitized_ai_endpoint("https://api.openai.com.evil.example/v1"), "自托管（地址已隐藏）");
        assert_eq!(sanitized_ai_endpoint("not a url"), "自托管（地址已隐藏）");
    }

    #[test]
    fn reports_missing_credentials_without_exposing_values() {
        let info = build_support_info(SupportInfoInput {
            platform: "github",
            custom_url: None,
            credential_storage: None,
            ai_configured: false,
            ai_endpoint: "https://api.openai.com/v1",
            local_cache_available: false,
        });

        assert_eq!(info.credential_storage, "未配置");
        assert!(info.formatted.contains("AI 配置：未配置"));
        assert!(info.formatted.contains("本地评论缓存：不可用"));
    }
}

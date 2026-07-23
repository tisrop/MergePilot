use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

static ERROR_REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Error, Debug)]
pub enum AppError {
    #[error("HTTP error: {0}")]
    Http(reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not authenticated for platform: {0}")]
    NotAuthenticated(String),

    #[error("Platform API error: {0}")]
    Api(String),

    #[allow(dead_code)]
    #[error("Unsupported merge strategy for this platform: {0}")]
    UnsupportedStrategy(String),

    #[error("AI error: {0}")]
    Ai(String),

    #[error("Not implemented for this platform: {0}")]
    NotImplemented(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        // reqwest includes the full request URL in status and transport errors.
        // Gitee authenticates through an access_token query parameter, so never
        // let that URL cross the IPC boundary.
        Self::Http(error.without_url())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandErrorCode {
    Validation,
    Authentication,
    PermissionDenied,
    NotFound,
    Conflict,
    RateLimited,
    Network,
    Timeout,
    InvalidResponse,
    Storage,
    Unsupported,
    Ai,
    Platform,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
    pub retryable: bool,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

pub type CommandResult<T> = Result<T, CommandError>;

impl CommandError {
    fn new(
        code: CommandErrorCode,
        message: impl Into<String>,
        retryable: bool,
        http_status: Option<u16>,
        source: &'static str,
    ) -> Self {
        let error = Self { code, message: message.into(), retryable, request_id: next_error_request_id(), http_status };
        error.log(source);
        error
    }

    fn from_http_status(status: u16, source: &'static str) -> Self {
        let (code, message, retryable) = match status {
            400 | 405 | 411 | 413 | 414 | 415 | 422 => {
                (CommandErrorCode::Validation, "请求内容未通过代码平台校验", false)
            }
            401 => (CommandErrorCode::Authentication, "登录凭据已失效，请重新登录", false),
            403 => (CommandErrorCode::PermissionDenied, "当前 Token 没有执行此操作的权限", false),
            404 => (CommandErrorCode::NotFound, "请求的远端资源不存在或当前 Token 无权访问", false),
            408 | 504 => (CommandErrorCode::Timeout, "代码平台请求超时，请稍后重试", true),
            409 => (CommandErrorCode::Conflict, "远端状态已变化，请刷新后重试", false),
            429 => (CommandErrorCode::RateLimited, "代码平台请求过于频繁，请稍后重试", true),
            500..=599 => (CommandErrorCode::Platform, "代码平台服务暂时不可用，请稍后重试", true),
            _ => (CommandErrorCode::Platform, "代码平台请求失败", false),
        };
        Self::new(code, message, retryable, Some(status), source)
    }

    fn from_ai_status(status: u16) -> Self {
        let (message, retryable) = match status {
            400 | 422 => ("AI 请求参数未被服务接受，请检查模型和评审设置", false),
            401 => ("AI API Key 无效或已失效，请检查 AI 设置", false),
            403 => ("AI 服务拒绝了请求，请检查 API Key 和 endpoint 权限", false),
            408 | 504 => ("AI 请求超时，请稍后重试", true),
            429 => ("AI 服务请求过于频繁，请稍后重试", true),
            500..=599 => ("AI 服务暂时不可用，请稍后重试", true),
            _ => ("AI 请求失败，请检查 AI 设置后重试", false),
        };
        Self::new(CommandErrorCode::Ai, message, retryable, Some(status), "ai")
    }

    fn status_from_message(message: &str) -> Option<u16> {
        fn parse_status_token(value: &str) -> Option<u16> {
            let value = value.trim_start_matches(|character: char| {
                character.is_whitespace() || matches!(character, ':' | '=' | '(' | '[' | '{' | '（')
            });
            let bytes = value.as_bytes();
            if bytes.len() < 3 || !bytes[..3].iter().all(u8::is_ascii_digit) {
                return None;
            }
            if bytes.get(3).is_some_and(u8::is_ascii_digit) {
                return None;
            }
            let status = value[..3].parse::<u16>().ok()?;
            (100..=599).contains(&status).then_some(status)
        }

        fn status_after_keyword(message: &str, keyword: &str) -> Option<u16> {
            message.match_indices(keyword).find_map(|(index, _)| {
                let before = message[..index].chars().next_back();
                let after_index = index + keyword.len();
                let after = message[after_index..].chars().next();
                let has_word_boundaries = before.is_none_or(|character| !character.is_ascii_alphanumeric())
                    && after.is_none_or(|character| !character.is_ascii_alphanumeric());
                has_word_boundaries.then(|| parse_status_token(&message[after_index..])).flatten()
            })
        }

        let normalized = message.to_ascii_lowercase();
        parse_status_token(&normalized).or_else(|| {
            ["api error", "status code", "http", "api"]
                .into_iter()
                .find_map(|keyword| status_after_keyword(&normalized, keyword))
        })
    }

    fn safe_message(message: &str, fallback: &str) -> String {
        let trimmed = message.trim();
        let lower = trimmed.to_ascii_lowercase();
        let contains_sensitive_context = lower.contains("://")
            || lower.contains("access_token=")
            || lower.contains("authorization:")
            || lower.contains("private-token:")
            || lower.contains("bearer ");
        if trimmed.is_empty() || contains_sensitive_context {
            return fallback.to_string();
        }

        let mut chars = trimmed.chars();
        let shortened = chars.by_ref().take(500).collect::<String>();
        if chars.next().is_some() {
            format!("{shortened}...")
        } else {
            shortened
        }
    }

    fn log(&self, source: &'static str) {
        let event = serde_json::json!({
            "event": "command_error",
            "source": source,
            "request_id": self.request_id,
            "code": self.code,
            "retryable": self.retryable,
            "http_status": self.http_status,
        });
        eprintln!("{event}");
    }
}

fn next_error_request_id() -> String {
    let timestamp =
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().try_into().unwrap_or(u64::MAX);
    let sequence = ERROR_REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("err-{timestamp:016x}-{sequence:016x}")
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Http(error) => {
                if let Some(status) = error.status() {
                    Self::from_http_status(status.as_u16(), "http")
                } else if error.is_timeout() {
                    Self::new(CommandErrorCode::Timeout, "网络请求超时，请稍后重试", true, None, "http")
                } else if error.is_connect() {
                    Self::new(CommandErrorCode::Network, "无法连接到远端服务，请检查网络", true, None, "http")
                } else {
                    Self::new(CommandErrorCode::Network, "网络请求失败，请稍后重试", true, None, "http")
                }
            }
            AppError::Json(_) => {
                Self::new(CommandErrorCode::InvalidResponse, "远端响应格式无效，请稍后重试", true, None, "json")
            }
            AppError::Io(_) => Self::new(CommandErrorCode::Storage, "本地数据访问失败", false, None, "io"),
            AppError::NotAuthenticated(_) => Self::new(
                CommandErrorCode::Authentication,
                "当前平台尚未登录或登录凭据已失效",
                false,
                None,
                "authentication",
            ),
            AppError::Api(message) => {
                if let Some(status) = Self::status_from_message(&message) {
                    Self::from_http_status(status, "platform_api")
                } else {
                    Self::new(
                        CommandErrorCode::Platform,
                        Self::safe_message(&message, "代码平台请求失败"),
                        false,
                        None,
                        "platform_api",
                    )
                }
            }
            AppError::UnsupportedStrategy(message) | AppError::NotImplemented(message) => Self::new(
                CommandErrorCode::Unsupported,
                Self::safe_message(&message, "当前平台不支持该操作"),
                false,
                None,
                "unsupported",
            ),
            AppError::Ai(message) => {
                if let Some(status) = Self::status_from_message(&message) {
                    Self::from_ai_status(status)
                } else {
                    Self::new(CommandErrorCode::Ai, Self::safe_message(&message, "AI 请求失败"), false, None, "ai")
                }
            }
            AppError::Unknown(message) => Self::new(
                CommandErrorCode::Unknown,
                Self::safe_message(&message, "发生未知错误"),
                false,
                None,
                "unknown",
            ),
        }
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        let lower = message.to_ascii_lowercase();
        if let Some(status) = Self::status_from_message(&message) {
            return Self::from_http_status(status, "command");
        }
        let (code, retryable, source) =
            if lower.contains("限流") || lower.contains("请求过于频繁") || lower.contains("rate limit") {
                (CommandErrorCode::RateLimited, true, "command")
            } else if lower.contains("超时") || lower.contains("timeout") {
                (CommandErrorCode::Timeout, true, "command")
            } else if lower.contains("网络") || lower.contains("连接失败") {
                (CommandErrorCode::Network, true, "command")
            } else if lower.contains("没有权限") || lower.contains("permission denied") {
                (CommandErrorCode::PermissionDenied, false, "command")
            } else if lower.contains("不存在") || lower.contains("未找到") || lower.contains("not found") {
                (CommandErrorCode::NotFound, false, "command")
            } else if lower.contains("已变化") || lower.contains("已更新") || lower.contains("冲突") {
                (CommandErrorCode::Conflict, false, "command")
            } else if lower.contains("不支持") || lower.contains("not implemented") {
                (CommandErrorCode::Unsupported, false, "validation")
            } else if lower.contains("未登录") || lower.contains("not authenticated") {
                (CommandErrorCode::Authentication, false, "validation")
            } else if lower.contains("不能为空")
                || lower.contains("无效")
                || lower.contains("必须")
                || lower.contains("不能")
                || lower.contains("超出")
                || lower.contains("unknown platform")
            {
                (CommandErrorCode::Validation, false, "validation")
            } else {
                (CommandErrorCode::Unknown, lower.contains("稍后重试"), "command")
            };
        Self::new(code, Self::safe_message(&message, "操作失败"), retryable, None, source)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

// Keep legacy conversions available for non-command code while command boundaries migrate to CommandError.
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, CommandError, CommandErrorCode};

    #[test]
    fn classifies_platform_status_without_exposing_remote_details() {
        let error = CommandError::from(AppError::Api(
            "GitHub API 429 (https://api.github.com/repos/private?access_token=secret): rate limited".into(),
        ));

        assert_eq!(error.code, CommandErrorCode::RateLimited);
        assert_eq!(error.http_status, Some(429));
        assert!(error.retryable);
        assert_eq!(error.message, "代码平台请求过于频繁，请稍后重试");
        assert!(!error.message.contains("secret"));
        assert!(!error.message.contains("github.com"));
    }

    #[test]
    fn reads_only_explicit_http_status_positions() {
        let validation = CommandError::from(AppError::Api(
            "GitHub API 422 (https://api.github.com/repos/o/r/pulls/404): Validation Failed".into(),
        ));
        assert_eq!(validation.code, CommandErrorCode::Validation);
        assert_eq!(validation.http_status, Some(422));

        let url_only = CommandError::from(AppError::Api("请求 https://api.github.com/repos/o/r/pulls/404 失败".into()));
        assert_eq!(url_only.code, CommandErrorCode::Platform);
        assert_eq!(url_only.http_status, None);

        let ai_auth = CommandError::from(AppError::Ai("AI API error (401 Unauthorized): invalid key".into()));
        assert_eq!(ai_auth.code, CommandErrorCode::Ai);
        assert_eq!(ai_auth.http_status, Some(401));
    }

    #[test]
    fn keeps_ai_http_errors_in_the_ai_error_domain() {
        let unauthorized = CommandError::from(AppError::Ai("AI API error (401): invalid api key".into()));
        assert_eq!(unauthorized.code, CommandErrorCode::Ai);
        assert_eq!(unauthorized.http_status, Some(401));
        assert_eq!(unauthorized.message, "AI API Key 无效或已失效，请检查 AI 设置");

        let rate_limited = CommandError::from(AppError::Ai("AI API error (429): too many requests".into()));
        assert_eq!(rate_limited.code, CommandErrorCode::Ai);
        assert!(rate_limited.retryable);
        assert_eq!(rate_limited.message, "AI 服务请求过于频繁，请稍后重试");
    }

    #[test]
    fn preserves_safe_platform_guidance() {
        let error = CommandError::from(AppError::Api("GitLab MR 已更新，请刷新 Diff 后重新评论".into()));

        assert_eq!(error.code, CommandErrorCode::Platform);
        assert_eq!(error.message, "GitLab MR 已更新，请刷新 Diff 后重新评论");
        assert!(!error.retryable);
    }

    #[test]
    fn classifies_validation_strings() {
        let error = CommandError::from("仓库 owner 和名称不能为空".to_string());

        assert_eq!(error.code, CommandErrorCode::Validation);
        assert_eq!(error.http_status, None);
        assert_eq!(error.to_string(), "仓库 owner 和名称不能为空");
    }

    #[test]
    fn serializes_stable_error_contract() {
        let error = CommandError::from(AppError::NotAuthenticated("github".into()));
        let value = serde_json::to_value(error).unwrap();

        assert_eq!(value["code"], "authentication");
        assert_eq!(value["message"], "当前平台尚未登录或登录凭据已失效");
        assert_eq!(value["retryable"], false);
        assert!(value["request_id"]
            .as_str()
            .is_some_and(|request_id| { request_id.starts_with("err-") && request_id.len() == 37 }));
        assert!(value.get("http_status").is_none());
    }
}

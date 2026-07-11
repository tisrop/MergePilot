use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

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

    #[error("Unknown error: {0}")]
    Unknown(String),
}

// Allow converting AppError to String for Tauri command returns
impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

use crate::ai::config::AiConfigManager;
use crate::http_client::HttpClient;
use crate::vault::TokenVault;
use std::sync::Arc;

/// Application state shared across all Tauri commands
pub struct AppState {
    pub http_client: Arc<HttpClient>,
    pub token_vault: Arc<TokenVault>,
    pub ai_config: Arc<AiConfigManager>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            http_client: Arc::new(HttpClient::new()),
            token_vault: Arc::new(TokenVault::new()),
            ai_config: Arc::new(AiConfigManager::new()),
        }
    }
}

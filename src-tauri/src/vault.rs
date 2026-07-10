use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::AppError;

/// Token storage using a simple JSON config file (no OS keyring access).
/// File location: ~/.mergepilot/config.json
pub struct TokenVault;

impl Default for TokenVault {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenVault {
    pub fn new() -> Self {
        Self
    }

    /// Directory for config file storage.
    /// Uses $HOME/.mergepilot (reliable on macOS dev without entitlements).
    fn storage_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            let dir = PathBuf::from(home).join(".mergepilot");
            if std::fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }
        PathBuf::from(".mergepilot")
    }

    fn config_path() -> PathBuf {
        Self::storage_dir().join("config.json")
    }

    fn read_config() -> HashMap<String, String> {
        let path = Self::config_path();
        if !path.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    fn write_config(config: &HashMap<String, String>) -> Result<(), AppError> {
        let path = Self::config_path();
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&path, content).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to write config to {}: {}", path.display(), e),
            ))
        })?;
        Ok(())
    }

    pub fn store_token(&self, platform: &str, token: &str) -> Result<(), AppError> {
        let mut config = Self::read_config();
        config.insert(format!("token_{}", platform), token.to_string());
        Self::write_config(&config)
    }

    pub fn get_token(&self, platform: &str) -> Result<Option<String>, AppError> {
        let config = Self::read_config();
        Ok(config.get(&format!("token_{}", platform)).cloned())
    }

    // ── Custom base URL (for self-hosted GitLab / Gitee Enterprise) ──

    pub fn store_custom_url(&self, platform: &str, url: &str) -> Result<(), AppError> {
        let mut config = Self::read_config();
        config.insert(format!("url_{}", platform), url.to_string());
        Self::write_config(&config)
    }

    pub fn get_custom_url(&self, platform: &str) -> Option<String> {
        let config = Self::read_config();
        config.get(&format!("url_{}", platform)).cloned()
    }

    pub fn delete_custom_url(&self, platform: &str) -> Result<(), AppError> {
        let mut config = Self::read_config();
        config.remove(&format!("url_{}", platform));
        Self::write_config(&config)
    }

    pub fn delete_token(&self, platform: &str) -> Result<(), AppError> {
        let mut config = Self::read_config();
        config.remove(&format!("token_{}", platform));
        Self::write_config(&config)
    }
}

use std::path::PathBuf;

use crate::crypto;
use crate::error::AppError;
use crate::models::AiConfig;

const CONFIG_FILE: &str = "ai_config.json";

pub struct AiConfigManager {
    config_dir: PathBuf,
}

impl Default for AiConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AiConfigManager {
    pub fn new() -> Self {
        let config_dir = directories::ProjectDirs::from("com", "mergepilot", "MergePilot")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".mergepilot"));

        std::fs::create_dir_all(&config_dir).ok();
        Self { config_dir }
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE)
    }

    /// Read AI config from disk.
    /// The `api_key_encrypted` field is NOT decrypted here — call `get_api_key` separately.
    pub fn get_config(&self) -> Result<AiConfig, AppError> {
        let path = self.config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: AiConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(AiConfig {
                endpoint: "https://api.openai.com/v1".to_string(),
                model: "gpt-4o".to_string(),
                api_key_configured: false,
                api_key_encrypted: None,
                system_prompt: None,
                temperature: Some(0.3),
                max_tokens: Some(4096),
            })
        }
    }

    /// Write AI config to disk.
    /// `api_key_encrypted` is preserved as-is in the JSON.
    pub fn save_config(&self, config: &AiConfig) -> Result<(), AppError> {
        let path = self.config_path();
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Encrypt the API key and store it inside `ai_config.json`.
    /// After this call, the caller should also call `save_config` to persist.
    pub fn save_api_key(&self, plaintext: &str) -> Result<String, AppError> {
        let encrypted =
            crypto::encrypt(plaintext).map_err(|e| AppError::Ai(format!("Failed to encrypt API key: {}", e)))?;

        // Update the config file with the encrypted key
        let mut config = self.get_config()?;
        config.api_key_encrypted = Some(encrypted);
        config.api_key_configured = true;
        self.save_config(&config)?;

        Ok(plaintext.to_string())
    }

    /// Decrypt and return the API key from the config file.
    pub fn get_api_key(&self) -> Result<String, AppError> {
        let config = self.get_config()?;
        let encrypted = config.api_key_encrypted.ok_or_else(|| AppError::Ai("No API key configured".to_string()))?;

        crypto::decrypt(&encrypted).map_err(|e| AppError::Ai(format!("Failed to decrypt API key: {}", e)))
    }

    /// Remove the API key from the config file.
    #[allow(dead_code)]
    pub fn delete_api_key(&self) -> Result<(), AppError> {
        let mut config = self.get_config()?;
        config.api_key_encrypted = None;
        config.api_key_configured = false;
        self.save_config(&config)?;
        Ok(())
    }
}

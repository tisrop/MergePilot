use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::crypto::{decrypt, encrypt};
use crate::error::AppError;

const KEYRING_SERVICE: &str = "com.mergebeacon";
const LEGACY_KEYRING_SERVICE: &str = "com.mergepilot";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStorage {
    SystemKeyring,
    EncryptedFile,
}

/// Platform token storage backed by the system credential store, with an encrypted config fallback.
pub struct TokenVault {
    session_tokens: RwLock<HashMap<String, String>>,
}

impl Default for TokenVault {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenVault {
    pub fn new() -> Self {
        Self { session_tokens: RwLock::new(HashMap::new()) }
    }

    fn cached_token(&self, platform: &str) -> Option<String> {
        self.session_tokens.read().unwrap_or_else(|poisoned| poisoned.into_inner()).get(platform).cloned()
    }

    fn cache_token(&self, platform: &str, token: &str) {
        self.session_tokens
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(platform.to_string(), token.to_string());
    }

    fn remove_cached_token(&self, platform: &str) {
        self.session_tokens.write().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(platform);
    }

    fn storage_dir() -> Result<PathBuf, AppError> {
        let dir =
            Self::storage_dir_path().ok_or_else(|| AppError::Unknown("无法确定当前用户的凭证存储目录".to_string()))?;
        fs::create_dir_all(&dir)?;
        set_dir_permissions(&dir)?;
        Ok(dir)
    }

    fn storage_dir_path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".mergebeacon"))
    }

    fn legacy_config_path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".mergepilot/config.json"))
    }

    fn config_path() -> Result<PathBuf, AppError> {
        Ok(Self::storage_dir()?.join("config.json"))
    }

    fn read_config() -> Result<HashMap<String, String>, AppError> {
        let path = Self::config_path()?;
        if !path.exists() {
            if let Some(legacy_path) = Self::legacy_config_path().filter(|path| path.exists()) {
                let content = fs::read_to_string(&legacy_path)?;
                let config: HashMap<String, String> = serde_json::from_str(&content)?;
                Self::write_config(&config)?;
                fs::remove_file(legacy_path)?;
                return Ok(config);
            }
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn write_config(config: &HashMap<String, String>) -> Result<(), AppError> {
        let path = Self::config_path()?;
        let temp_path = path.with_extension(format!("json.tmp.{}", std::process::id()));
        let content = serde_json::to_vec_pretty(config)?;
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);
            set_file_creation_mode(&mut options);
            let mut file = options.open(&temp_path)?;
            file.write_all(&content)?;
            file.flush()?;
            file.sync_all()?;
            fs::rename(&temp_path, &path)?;
            set_file_permissions(&path)?;
            if let Some(parent) = path.parent() {
                File::open(parent)?.sync_all()?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    fn keyring_is_persistent() -> bool {
        use keyring::credential::CredentialPersistence;

        matches!(keyring::default::default_credential_builder().persistence(), CredentialPersistence::UntilDelete)
    }

    fn keyring_entry_for(service: &str, platform: &str) -> Result<keyring::Entry, keyring::Error> {
        keyring::Entry::new(service, &format!("git-platform:{platform}"))
    }

    fn keyring_entry(platform: &str) -> Result<keyring::Entry, keyring::Error> {
        Self::keyring_entry_for(KEYRING_SERVICE, platform)
    }

    fn legacy_keyring_entry(platform: &str) -> Result<keyring::Entry, keyring::Error> {
        Self::keyring_entry_for(LEGACY_KEYRING_SERVICE, platform)
    }

    fn store_encrypted(platform: &str, token: &str) -> Result<(), AppError> {
        let mut config = Self::read_config()?;
        let encrypted = encrypt(token).map_err(AppError::Unknown)?;
        config.insert(format!("token_encrypted_{platform}"), encrypted);
        config.remove(&format!("token_{platform}"));
        Self::write_config(&config)
    }

    pub fn store_token(&self, platform: &str, token: &str) -> Result<CredentialStorage, AppError> {
        if Self::keyring_is_persistent() {
            if let Ok(entry) = Self::keyring_entry(platform) {
                if entry.set_password(token).is_ok() {
                    let mut config = Self::read_config()?;
                    config.remove(&format!("token_encrypted_{platform}"));
                    config.remove(&format!("token_{platform}"));
                    Self::write_config(&config)?;
                    self.cache_token(platform, token);
                    return Ok(CredentialStorage::SystemKeyring);
                }
            }
        }
        Self::store_encrypted(platform, token)?;
        self.cache_token(platform, token);
        Ok(CredentialStorage::EncryptedFile)
    }

    pub fn get_token(&self, platform: &str) -> Result<Option<String>, AppError> {
        if let Some(token) = self.cached_token(platform) {
            return Ok(Some(token));
        }

        if Self::keyring_is_persistent() {
            if let Ok(entry) = Self::keyring_entry(platform) {
                match entry.get_password() {
                    Ok(token) => {
                        self.cache_token(platform, &token);
                        return Ok(Some(token));
                    }
                    Err(keyring::Error::NoEntry) => {}
                    Err(_) => {}
                }
            }
            if let Ok(legacy_entry) = Self::legacy_keyring_entry(platform) {
                if let Ok(token) = legacy_entry.get_password() {
                    self.store_token(platform, &token)?;
                    legacy_entry.delete_credential().map_err(|error| {
                        AppError::Unknown(format!("Failed to remove legacy token from system keyring: {error}"))
                    })?;
                    return Ok(Some(token));
                }
            }
        }

        let config = Self::read_config()?;
        if let Some(value) = config.get(&format!("token_encrypted_{platform}")) {
            let token = decrypt(value).map_err(AppError::Unknown)?;
            self.cache_token(platform, &token);
            return Ok(Some(token));
        }

        if let Some(token) = config.get(&format!("token_{platform}")).cloned() {
            self.store_token(platform, &token)?;
            return Ok(Some(token));
        }
        Ok(None)
    }

    pub fn credential_storage(&self, platform: &str) -> Result<Option<CredentialStorage>, AppError> {
        let config = Self::read_config()?;
        if config.contains_key(&format!("token_encrypted_{platform}"))
            || config.contains_key(&format!("token_{platform}"))
        {
            return Ok(Some(CredentialStorage::EncryptedFile));
        }

        if Self::keyring_is_persistent() {
            if let Ok(entry) = Self::keyring_entry(platform) {
                if entry.get_password().is_ok() {
                    return Ok(Some(CredentialStorage::SystemKeyring));
                }
            }
        }

        Ok(None)
    }

    pub fn store_custom_url(&self, platform: &str, url: &str) -> Result<(), AppError> {
        let mut config = Self::read_config()?;
        config.insert(format!("url_{platform}"), url.to_string());
        Self::write_config(&config)
    }

    pub fn get_custom_url(&self, platform: &str) -> Option<String> {
        Self::read_config().ok().and_then(|config| config.get(&format!("url_{platform}")).cloned())
    }

    pub fn delete_custom_url(&self, platform: &str) -> Result<(), AppError> {
        let mut config = Self::read_config()?;
        config.remove(&format!("url_{platform}"));
        Self::write_config(&config)
    }

    pub fn delete_token(&self, platform: &str) -> Result<(), AppError> {
        self.remove_cached_token(platform);
        let keyring_error = match Self::keyring_entry(platform) {
            Ok(entry) => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => None,
                Err(error) => Some(error.to_string()),
            },
            Err(error) => Some(error.to_string()),
        };
        if let Ok(entry) = Self::legacy_keyring_entry(platform) {
            if let Err(error) = entry.delete_credential() {
                if !matches!(error, keyring::Error::NoEntry) && keyring_error.is_none() {
                    return Err(AppError::Unknown(format!(
                        "Failed to remove legacy token from system keyring: {error}"
                    )));
                }
            }
        }
        let mut config = Self::read_config()?;
        config.remove(&format!("token_encrypted_{platform}"));
        config.remove(&format!("token_{platform}"));
        Self::write_config(&config)?;
        if let Some(error) = keyring_error {
            return Err(AppError::Unknown(format!("Failed to remove token from system keyring: {error}")));
        }
        Ok(())
    }
}

#[cfg(unix)]
fn set_dir_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(unix)]
fn set_file_creation_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_file_creation_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_file_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TokenVault;

    #[test]
    fn configured_keyring_backend_persists_across_processes() {
        assert!(TokenVault::keyring_is_persistent());
    }

    #[test]
    fn storage_dir_uses_platform_user_home_instead_of_process_working_directory() {
        let storage_dir = TokenVault::storage_dir_path().expect("platform user home should exist");
        let base_dirs = directories::BaseDirs::new().expect("platform user directories should exist");

        assert_eq!(storage_dir, base_dirs.home_dir().join(".mergebeacon"));
        assert_ne!(storage_dir, std::path::PathBuf::from(".mergebeacon"));
    }

    #[test]
    fn session_token_cache_is_available_without_persistent_storage() {
        let vault = TokenVault::new();
        vault.cache_token("test-platform", "secret-token");

        assert_eq!(vault.get_token("test-platform").unwrap().as_deref(), Some("secret-token"));

        vault.remove_cached_token("test-platform");
        assert_eq!(vault.cached_token("test-platform"), None);
    }
}

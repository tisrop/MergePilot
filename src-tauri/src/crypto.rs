use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Derive a 32-byte encryption key from:
/// - A fixed application secret
/// - The OS username (from $USER / $USERNAME)
///
/// This ensures that even if someone copies the config file to another machine
/// with a different username, the data cannot be decrypted.
// These legacy identifiers are part of the persisted ciphertext format, not display branding.
fn derive_key() -> [u8; 32] {
    let app_secret: &[u8] = b"mergepilot-aes256-v1-ae7f3c9d";

    let username =
        std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "mergepilot-user".to_string());

    let mut hasher = Sha256::new();
    hasher.update(app_secret);
    hasher.update(username.as_bytes());

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt plaintext using AES-256-GCM.
/// Returns base64-encoded (nonce || ciphertext).
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let key_bytes = derive_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let cipher = Aes256Gcm::new(key);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).map_err(|e| format!("Encryption failed: {}", e))?;

    // nonce (12 bytes) || ciphertext || GCM tag (16 bytes, appended by aes-gcm)
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(base64_encode(&combined))
}

/// Decrypt base64-encoded (nonce || ciphertext) using AES-256-GCM.
pub fn decrypt(encoded: &str) -> Result<String, String> {
    let combined = base64_decode(encoded)?;
    if combined.len() < 12 + 16 {
        // minimum: 12-byte nonce + 16-byte GCM tag
        return Err("Invalid ciphertext: too short".to_string());
    }

    let key_bytes = derive_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(key);
    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| format!("Decryption failed: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {}", e))
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|e| format!("Base64 decode error: {}", e))
}

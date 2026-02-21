//! Unified secret resolution from multiple sources.
//!
//! This module provides utilities for resolving secrets from multiple sources
//! in priority order, supporting flexible deployment scenarios:
//!
//! 1. **Direct value** - For quick local testing (e.g., `password: "mypassword"`)
//! 2. **File reference** - For Docker secrets pattern (e.g., `passwordFile: /run/secrets/password`)
//! 3. **Env var reference** - For Kubernetes/production (e.g., `passwordEnvVar: GMAIL_APP_PASSWORD`)

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use secrecy::SecretString;
use std::fs;

/// Error type for secret resolution failures.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("No secret source provided (need one of: direct value, file path, or env var name)")]
    NoSourceProvided,

    #[error("Failed to read secret from file '{path}': {source}")]
    FileReadError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Environment variable '{name}' not set")]
    EnvVarNotSet { name: String },

    #[error("Environment variable '{name}' contains invalid UTF-8")]
    EnvVarNotUnicode { name: String },

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Invalid encryption key: {0}")]
    InvalidKey(String),
}

/// Result type for secret resolution.
pub type Result<T> = std::result::Result<T, SecretError>;

/// Resolves a secret from multiple sources in priority order:
/// 1. Direct value (if provided and non-empty)
/// 2. File contents (if path provided)
/// 3. Environment variable (if name provided)
///
/// # Arguments
///
/// * `direct` - Optional direct value of the secret
/// * `file_path` - Optional path to a file containing the secret
/// * `env_var` - Optional name of an environment variable containing the secret
///
/// # Returns
///
/// The resolved secret wrapped in `SecretString`, or an error if no source
/// provides a valid value.
///
/// # Examples
///
/// ```ignore
/// use paporg::secrets::resolve_secret;
///
/// // Direct value takes priority
/// let secret = resolve_secret(
///     Some("my-password"),
///     Some("/run/secrets/password"),
///     Some("PASSWORD_ENV"),
/// )?;
///
/// // File path used if no direct value
/// let secret = resolve_secret(
///     None,
///     Some("~/.secrets/token"),
///     Some("TOKEN_ENV"),
/// )?;
///
/// // Env var used as fallback
/// let secret = resolve_secret(
///     None,
///     None,
///     Some("API_KEY"),
/// )?;
/// ```
pub fn resolve_secret(
    direct: Option<&str>,
    file_path: Option<&str>,
    env_var: Option<&str>,
) -> Result<SecretString> {
    // Priority 1: Direct value
    if let Some(value) = direct {
        if !value.is_empty() {
            return Ok(SecretString::from(value.to_string()));
        }
    }

    // Priority 2: File
    if let Some(path) = file_path {
        if !path.is_empty() {
            let expanded = expand_home(path);
            match fs::read_to_string(&expanded) {
                Ok(content) => return Ok(SecretString::from(content.trim().to_string())),
                Err(e) => {
                    return Err(SecretError::FileReadError {
                        path: expanded,
                        source: e,
                    })
                }
            }
        }
    }

    // Priority 3: Environment variable
    if let Some(var_name) = env_var {
        if !var_name.is_empty() {
            match std::env::var(var_name) {
                Ok(value) => {
                    // Trim whitespace for consistency (env vars may have trailing newlines)
                    let trimmed = value.trim();
                    return Ok(SecretString::from(trimmed));
                }
                Err(std::env::VarError::NotPresent) => {
                    return Err(SecretError::EnvVarNotSet {
                        name: var_name.to_string(),
                    })
                }
                Err(std::env::VarError::NotUnicode(_)) => {
                    return Err(SecretError::EnvVarNotUnicode {
                        name: var_name.to_string(),
                    })
                }
            }
        }
    }

    Err(SecretError::NoSourceProvided)
}

/// Resolves a secret, returning None if no source is provided instead of an error.
///
/// This is useful for optional secrets where missing values are acceptable.
pub fn resolve_secret_optional(
    direct: Option<&str>,
    file_path: Option<&str>,
    env_var: Option<&str>,
) -> Result<Option<SecretString>> {
    match resolve_secret(direct, file_path, env_var) {
        Ok(secret) => Ok(Some(secret)),
        Err(SecretError::NoSourceProvided) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Checks if at least one secret source is configured (non-empty).
///
/// This is useful for validation to ensure the user has provided
/// at least one way to obtain a secret.
pub fn has_secret_source(
    direct: Option<&str>,
    file_path: Option<&str>,
    env_var: Option<&str>,
) -> bool {
    direct.is_some_and(|s| !s.is_empty())
        || file_path.is_some_and(|s| !s.is_empty())
        || env_var.is_some_and(|s| !s.is_empty())
}

/// Expands `~` to the user's home directory.
///
/// Works cross-platform: checks HOME (Unix) then USERPROFILE (Windows).
/// Handles both `~/path` and standalone `~`.
///
/// **Limitation**: This function does NOT support `~user/path` syntax
/// (e.g., `~alice/documents`). Only the current user's home directory
/// expansion via `~` or `~/path` is supported. Use absolute paths if
/// you need to reference other users' directories.
fn expand_home(path: &str) -> String {
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            if path == "~" {
                return home.to_string_lossy().into_owned();
            }
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

// ============================================
// Token Encryption
// ============================================

/// Encryption key environment variable name.
pub const TOKEN_KEY_ENV_VAR: &str = "PAPORG_TOKEN_KEY";

/// Nonce size for AES-256-GCM (96 bits = 12 bytes).
const NONCE_SIZE: usize = 12;

/// Token encryptor using AES-256-GCM.
///
/// Reads the encryption key from the `PAPORG_TOKEN_KEY` environment variable.
/// The key must be a 64-character hex string (32 bytes).
pub struct TokenEncryptor {
    cipher: Aes256Gcm,
}

impl TokenEncryptor {
    /// Creates a new TokenEncryptor from the environment variable.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `PAPORG_TOKEN_KEY` is not set
    /// - The key is not a valid 64-character hex string
    pub fn from_env() -> Result<Self> {
        let key_hex = std::env::var(TOKEN_KEY_ENV_VAR).map_err(|_| {
            SecretError::InvalidKey(format!(
                "Environment variable {} not set",
                TOKEN_KEY_ENV_VAR
            ))
        })?;

        Self::from_hex_key(&key_hex)
    }

    /// Creates a new TokenEncryptor from a hex-encoded key.
    ///
    /// # Arguments
    ///
    /// * `key_hex` - A 64-character hex string (32 bytes decoded)
    pub fn from_hex_key(key_hex: &str) -> Result<Self> {
        let key_bytes = hex_decode(key_hex)
            .map_err(|e| SecretError::InvalidKey(format!("Invalid hex key: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(SecretError::InvalidKey(format!(
                "Key must be 32 bytes (64 hex chars), got {} bytes",
                key_bytes.len()
            )));
        }

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| SecretError::InvalidKey(format!("Failed to create cipher: {}", e)))?;

        Ok(Self { cipher })
    }

    /// Encrypts plaintext and returns hex-encoded ciphertext with prepended nonce.
    ///
    /// Format: `<12-byte nonce><ciphertext>` (all hex-encoded)
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce_bytes = rand_bytes::<NONCE_SIZE>()?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| SecretError::EncryptionError(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut combined = nonce_bytes.to_vec();
        combined.extend(ciphertext);

        Ok(hex_encode(&combined))
    }

    /// Decrypts hex-encoded ciphertext (with prepended nonce) and returns plaintext.
    pub fn decrypt(&self, ciphertext_hex: &str) -> Result<String> {
        let combined = hex_decode(ciphertext_hex)
            .map_err(|e| SecretError::DecryptionError(format!("Invalid hex: {}", e)))?;

        if combined.len() < NONCE_SIZE {
            return Err(SecretError::DecryptionError(
                "Ciphertext too short".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext_bytes = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| SecretError::DecryptionError(e.to_string()))?;

        String::from_utf8(plaintext_bytes)
            .map_err(|e| SecretError::DecryptionError(format!("Invalid UTF-8: {}", e)))
    }
}

/// Encodes bytes as lowercase hex string.
///
/// Uses a single allocation and direct character writing for efficiency.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    result
}

/// Decodes hex string to bytes.
fn hex_decode(hex: &str) -> std::result::Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("Hex string must have even length".to_string());
    }

    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at position {}: {}", i, e))
        })
        .collect()
}

/// Generates random bytes using getrandom.
///
/// Returns an error if the system's random number generator fails.
fn rand_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    // Use getrandom for cryptographically secure random bytes
    // This is available on all platforms including WASM
    getrandom::getrandom(&mut bytes).map_err(|e| {
        SecretError::EncryptionError(format!("Failed to generate random bytes: {}", e))
    })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use serial_test::serial;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Tests that modify environment variables must run serially to avoid race conditions
    #[test]
    #[serial]
    fn test_direct_value_takes_priority() {
        std::env::set_var("TEST_SECRET_1", "env_value");
        let result = resolve_secret(Some("direct_value"), None, Some("TEST_SECRET_1")).unwrap();
        assert_eq!(result.expose_secret(), "direct_value");
        std::env::remove_var("TEST_SECRET_1");
    }

    #[test]
    #[serial]
    fn test_file_takes_priority_over_env() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "file_value").unwrap();

        std::env::set_var("TEST_SECRET_2", "env_value");
        let result = resolve_secret(
            None,
            Some(temp_file.path().to_str().unwrap()),
            Some("TEST_SECRET_2"),
        )
        .unwrap();
        assert_eq!(result.expose_secret(), "file_value");
        std::env::remove_var("TEST_SECRET_2");
    }

    #[test]
    #[serial]
    fn test_env_var_fallback() {
        std::env::set_var("TEST_SECRET_3", "env_value");
        let result = resolve_secret(None, None, Some("TEST_SECRET_3")).unwrap();
        assert_eq!(result.expose_secret(), "env_value");
        std::env::remove_var("TEST_SECRET_3");
    }

    #[test]
    fn test_no_source_error() {
        let result = resolve_secret(None, None, None);
        assert!(matches!(result, Err(SecretError::NoSourceProvided)));
    }

    #[test]
    #[serial]
    fn test_empty_strings_ignored() {
        std::env::set_var("TEST_SECRET_4", "env_value");
        let result = resolve_secret(Some(""), Some(""), Some("TEST_SECRET_4")).unwrap();
        assert_eq!(result.expose_secret(), "env_value");
        std::env::remove_var("TEST_SECRET_4");
    }

    #[test]
    fn test_file_not_found_error() {
        let result = resolve_secret(None, Some("/nonexistent/path/to/secret"), None);
        assert!(matches!(result, Err(SecretError::FileReadError { .. })));
    }

    #[test]
    fn test_env_var_not_set_error() {
        let result = resolve_secret(None, None, Some("DEFINITELY_NOT_SET_VAR_12345"));
        assert!(matches!(result, Err(SecretError::EnvVarNotSet { .. })));
    }

    #[test]
    fn test_file_content_trimmed() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "  secret_with_whitespace  ").unwrap();

        let result = resolve_secret(None, Some(temp_file.path().to_str().unwrap()), None).unwrap();
        assert_eq!(result.expose_secret(), "secret_with_whitespace");
    }

    #[test]
    fn test_has_secret_source() {
        assert!(has_secret_source(Some("value"), None, None));
        assert!(has_secret_source(None, Some("/path"), None));
        assert!(has_secret_source(None, None, Some("ENV_VAR")));
        assert!(!has_secret_source(None, None, None));
        assert!(!has_secret_source(Some(""), Some(""), Some("")));
    }

    #[test]
    #[serial]
    fn test_resolve_secret_optional() {
        // Returns None when no source provided
        let result = resolve_secret_optional(None, None, None).unwrap();
        assert!(result.is_none());

        // Returns Some when source provided
        std::env::set_var("TEST_SECRET_OPT", "value");
        let result = resolve_secret_optional(None, None, Some("TEST_SECRET_OPT")).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().expose_secret(), "value");
        std::env::remove_var("TEST_SECRET_OPT");
    }

    #[test]
    #[serial]
    fn test_expand_home() {
        // Test that non-home paths are unchanged
        assert_eq!(expand_home("/absolute/path"), "/absolute/path");
        assert_eq!(expand_home("relative/path"), "relative/path");

        // Test home expansion (only if HOME is set)
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(expand_home("~/test"), format!("{}/test", home));
            // Test standalone ~ expansion
            assert_eq!(expand_home("~"), home);
        }
    }

    // ============================================
    // Token Encryption Tests
    // ============================================

    // Test key: 32 bytes = 64 hex chars
    const TEST_KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn test_token_encryptor_roundtrip() {
        let encryptor = TokenEncryptor::from_hex_key(TEST_KEY).unwrap();
        let plaintext = "my-secret-token-12345";

        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_token_encryptor_different_ciphertext_each_time() {
        let encryptor = TokenEncryptor::from_hex_key(TEST_KEY).unwrap();
        let plaintext = "same-plaintext";

        let ciphertext1 = encryptor.encrypt(plaintext).unwrap();
        let ciphertext2 = encryptor.encrypt(plaintext).unwrap();

        // Same plaintext should produce different ciphertext due to random nonce
        assert_ne!(ciphertext1, ciphertext2);

        // But both should decrypt to the same plaintext
        assert_eq!(encryptor.decrypt(&ciphertext1).unwrap(), plaintext);
        assert_eq!(encryptor.decrypt(&ciphertext2).unwrap(), plaintext);
    }

    #[test]
    fn test_token_encryptor_invalid_key_length() {
        // Too short
        let result = TokenEncryptor::from_hex_key("0123456789abcdef");
        assert!(matches!(result, Err(SecretError::InvalidKey(_))));

        // Too long
        let result = TokenEncryptor::from_hex_key(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00",
        );
        assert!(matches!(result, Err(SecretError::InvalidKey(_))));
    }

    #[test]
    fn test_token_encryptor_invalid_hex_key() {
        let result = TokenEncryptor::from_hex_key("not-valid-hex-string-at-all!!!!!");
        assert!(matches!(result, Err(SecretError::InvalidKey(_))));
    }

    #[test]
    fn test_token_encryptor_decrypt_invalid_ciphertext() {
        let encryptor = TokenEncryptor::from_hex_key(TEST_KEY).unwrap();

        // Invalid hex
        let result = encryptor.decrypt("not-hex!");
        assert!(matches!(result, Err(SecretError::DecryptionError(_))));

        // Too short (less than nonce size)
        let result = encryptor.decrypt("aabbccdd");
        assert!(matches!(result, Err(SecretError::DecryptionError(_))));

        // Valid hex but tampered ciphertext
        let ciphertext = encryptor.encrypt("test").unwrap();
        let mut tampered = hex_decode(&ciphertext).unwrap();
        if let Some(byte) = tampered.last_mut() {
            *byte ^= 0xff; // Flip bits
        }
        let tampered_hex = hex_encode(&tampered);
        let result = encryptor.decrypt(&tampered_hex);
        assert!(matches!(result, Err(SecretError::DecryptionError(_))));
    }

    #[test]
    fn test_hex_encode_decode_roundtrip() {
        let original = vec![0x00, 0xff, 0x12, 0xab, 0xcd, 0xef];
        let encoded = hex_encode(&original);
        assert_eq!(encoded, "00ff12abcdef");

        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_hex_decode_errors() {
        // Odd length
        assert!(hex_decode("abc").is_err());

        // Invalid characters
        assert!(hex_decode("ghij").is_err());
    }

    #[test]
    fn test_token_encryptor_empty_plaintext() {
        let encryptor = TokenEncryptor::from_hex_key(TEST_KEY).unwrap();

        let ciphertext = encryptor.encrypt("").unwrap();
        let decrypted = encryptor.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_token_encryptor_unicode_plaintext() {
        let encryptor = TokenEncryptor::from_hex_key(TEST_KEY).unwrap();
        let plaintext = "Hello, 世界! 🔐 émojis and ünïcödé";

        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}

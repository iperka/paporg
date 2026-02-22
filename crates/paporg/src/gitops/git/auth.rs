//! Git authentication handling.

use std::path::PathBuf;

use crate::gitops::error::{GitOpsError, Result};
use crate::gitops::resource::{GitAuthSettings, GitAuthType};

/// Escapes a token for safe use in single-quoted shell strings.
/// Replaces single quotes with '\'' (end quote, escaped quote, start quote).
pub fn shell_escape_token(token: &str) -> String {
    token.replace('\'', "'\\''")
}

/// Escapes a token for safe use in Windows batch scripts.
/// Escapes batch metacharacters that could lead to command injection.
#[cfg(windows)]
fn escape_token_for_windows_batch(token: &str) -> String {
    let mut escaped = String::with_capacity(token.len() * 2);
    for ch in token.chars() {
        match ch {
            '%' => escaped.push_str("%%"),
            '^' | '&' | '|' | '<' | '>' | '(' | ')' | '"' => {
                escaped.push('^');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// RAII guard for askpass script cleanup.
///
/// Automatically deletes the askpass script file when dropped, ensuring
/// sensitive tokens are not left on disk even if an error occurs.
#[derive(Debug)]
pub struct AskpassCleanup {
    path: Option<PathBuf>,
}

impl AskpassCleanup {
    /// Creates a new cleanup guard for the given path.
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Creates an empty guard that does nothing on drop.
    pub(crate) fn empty() -> Self {
        Self { path: None }
    }
}

impl Drop for AskpassCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                // Log but don't panic - best effort cleanup
                log::warn!("Failed to clean up askpass script: {}", e);
            }
        }
    }
}

/// Authentication environment for git commands.
#[derive(Debug)]
pub struct AuthEnv {
    /// Environment variables to set for the git command.
    pub env_vars: Vec<(String, String)>,
    /// RAII guard — must outlive the git command to keep the askpass script alive.
    pub _cleanup: AskpassCleanup,
}

/// Build auth environment from GitAuthSettings.
///
/// Returns an `AuthEnv` containing the environment variables to pass to git
/// and a cleanup guard that deletes any temporary askpass scripts on drop.
pub fn build_auth_env(auth: &GitAuthSettings) -> Result<AuthEnv> {
    use secrecy::ExposeSecret;

    let mut env = Vec::new();

    match auth.auth_type {
        GitAuthType::None => {
            return Ok(AuthEnv {
                env_vars: env,
                _cleanup: AskpassCleanup::empty(),
            });
        }
        GitAuthType::Token => {
            // Get token from configured sources (direct, file, or env var)
            let env_var = if auth.token_env_var.is_empty() {
                None
            } else {
                Some(auth.token_env_var.as_str())
            };

            let token = crate::secrets::resolve_secret(
                auth.token_insecure.as_deref(),
                auth.token_file.as_deref(),
                env_var,
            )
            .map_err(|e| {
                GitOpsError::GitAuthFailed(format!(
                    "Failed to resolve git token: {}. Configure token, tokenFile, or tokenEnvVar.",
                    e
                ))
            })?;

            // Use git credential helper or URL with token
            // For HTTPS URLs, we can use GIT_ASKPASS
            // Shell-escape the token to prevent command injection
            let escaped_token = shell_escape_token(token.expose_secret());

            // Create temp script with cryptographically secure random filename
            let temp_dir = std::env::temp_dir();
            let random_suffix = uuid::Uuid::new_v4().to_string();

            // Platform-specific script creation
            #[cfg(unix)]
            let (askpass_path, askpass_script) = {
                let askpass_filename = format!(".git-askpass-{}.sh", random_suffix);
                let path = temp_dir.join(&askpass_filename);
                let script = format!(
                    r#"#!/bin/sh
echo '{}'"#,
                    escaped_token
                );
                (path, script)
            };

            #[cfg(windows)]
            let (askpass_path, askpass_script) = {
                let askpass_filename = format!(".git-askpass-{}.bat", random_suffix);
                let path = temp_dir.join(&askpass_filename);
                let escaped = escape_token_for_windows_batch(token.expose_secret());
                let script = format!("@echo off\r\necho {}\r\n", escaped);
                (path, script)
            };

            // Write with restrictive permissions from the start on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true) // Fail if file exists (atomic creation)
                    .mode(0o700) // Owner read/write/execute only
                    .open(&askpass_path)?;
                std::io::Write::write_all(&mut file, askpass_script.as_bytes())?;
            }

            #[cfg(not(unix))]
            {
                std::fs::write(&askpass_path, &askpass_script)?;
            }

            // Create cleanup guard before adding to env
            let cleanup = AskpassCleanup::new(askpass_path.clone());

            let askpass_path_str = askpass_path
                .to_str()
                .ok_or_else(|| {
                    GitOpsError::GitAuthFailed(
                        "Temp directory path contains non-UTF8 characters".to_string(),
                    )
                })?
                .to_string();

            env.push(("GIT_ASKPASS".to_string(), askpass_path_str));
            env.push(("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()));

            return Ok(AuthEnv {
                env_vars: env,
                _cleanup: cleanup,
            });
        }
        GitAuthType::SshKey => {
            let key_path = if auth.ssh_key_path.is_empty() {
                dirs::home_dir()
                    .map(|h| h.join(".ssh").join("id_ed25519"))
                    .unwrap_or_else(|| PathBuf::from(".ssh/id_ed25519"))
            } else {
                let path = &auth.ssh_key_path;
                if path == "~" {
                    dirs::home_dir().unwrap_or_default()
                } else if let Some(rest) = path.strip_prefix("~/") {
                    dirs::home_dir()
                        .map(|h| h.join(rest))
                        .unwrap_or_else(|| PathBuf::from(path))
                } else {
                    PathBuf::from(path)
                }
            };

            if !key_path.exists() {
                return Err(GitOpsError::GitAuthFailed(format!(
                    "SSH key file not found: {}",
                    key_path.display()
                )));
            }

            // Shell-escape the key path to handle spaces and special characters
            let safe_path = {
                let display = key_path.display().to_string();
                let escaped = display.replace('\'', "'\\''");
                if escaped.starts_with('-') {
                    format!("'./{}'", escaped)
                } else {
                    format!("'{}'", escaped)
                }
            };

            // Use StrictHostKeyChecking=accept-new to accept new hosts but reject changed keys
            env.push((
                "GIT_SSH_COMMAND".to_string(),
                format!("ssh -i {} -o StrictHostKeyChecking=accept-new", safe_path),
            ));
        }
    }

    Ok(AuthEnv {
        env_vars: env,
        _cleanup: AskpassCleanup::empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_env_none() {
        let settings = GitAuthSettings::default();
        let auth = build_auth_env(&settings).unwrap();
        assert!(auth.env_vars.is_empty());
    }

    #[test]
    fn test_auth_env_ssh_key_not_found() {
        let settings = GitAuthSettings {
            auth_type: GitAuthType::SshKey,
            ssh_key_path: "/nonexistent/path/id_rsa".to_string(),
            ..Default::default()
        };
        let result = build_auth_env(&settings);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("SSH key file not found"));
    }

    #[test]
    fn test_auth_env_ssh_key_tilde_expansion() {
        // With tilde path that won't exist, we still get the error about file not found
        // but the path should be expanded
        let settings = GitAuthSettings {
            auth_type: GitAuthType::SshKey,
            ssh_key_path: "~/nonexistent_key".to_string(),
            ..Default::default()
        };
        let result = build_auth_env(&settings);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Should have expanded ~ and not contain the literal tilde
        assert!(!err.contains("~/"));
    }

    #[test]
    fn test_shell_escape_token() {
        assert_eq!(shell_escape_token("simple"), "simple");
        assert_eq!(shell_escape_token("it's"), "it'\\''s");
        assert_eq!(shell_escape_token("a'b'c"), "a'\\''b'\\''c");
    }

    #[test]
    fn test_auth_env_ssh_key_default_path() {
        // When ssh_key_path is empty, it defaults to ~/.ssh/id_ed25519
        // This will likely fail with "not found" on CI but tests the path logic
        let settings = GitAuthSettings {
            auth_type: GitAuthType::SshKey,
            ssh_key_path: String::new(),
            ..Default::default()
        };
        let result = build_auth_env(&settings);
        // Either succeeds (key exists) or fails with "not found" (expected on most systems)
        if let Err(e) = result {
            assert!(e.to_string().contains("SSH key file not found"));
        }
    }
}

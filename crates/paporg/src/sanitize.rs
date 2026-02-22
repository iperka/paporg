//! Helpers for sanitizing data before it enters tracing span attributes.
//!
//! Traces are safe to share for debugging — these functions ensure no
//! sensitive data (file paths, repo tokens, SSH keys) leaks into spans.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Returns only the filename component of a path (no directory).
///
/// Safe for span fields — reveals file name without exposing the full path.
pub fn redact_path(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>")
        .to_string()
}

/// Strips userinfo/tokens from a git remote URL.
///
/// - `https://ghp_token@github.com/user/repo` → `https://****@github.com/user/repo`
/// - `git@github.com:user/repo.git` → `git@github.com:user/repo.git` (no change)
/// - `https://github.com/user/repo` → `https://github.com/user/repo` (no change)
pub fn redact_repo_url(url: &str) -> String {
    // SSH URLs don't contain tokens
    if url.starts_with("git@") {
        return url.to_string();
    }

    // HTTPS URLs may contain tokens in userinfo: https://TOKEN@host/...
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let scheme = &url[..scheme_end + 3];
            let after_at = &after_scheme[at_pos + 1..];
            return format!("{}****@{}", scheme, after_at);
        }
    }

    url.to_string()
}

/// Returns a short deterministic hash of a path for correlation without
/// exposing the actual path.
pub fn hash_path(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_redact_path_returns_filename() {
        assert_eq!(
            redact_path(Path::new("/home/user/Documents/invoice.pdf")),
            "invoice.pdf"
        );
    }

    #[test]
    fn test_redact_path_no_filename() {
        assert_eq!(redact_path(Path::new("/")), "<unknown>");
    }

    #[test]
    fn test_redact_repo_url_https_with_token() {
        assert_eq!(
            redact_repo_url("https://ghp_xxxx@github.com/user/repo.git"),
            "https://****@github.com/user/repo.git"
        );
    }

    #[test]
    fn test_redact_repo_url_https_no_token() {
        assert_eq!(
            redact_repo_url("https://github.com/user/repo.git"),
            "https://github.com/user/repo.git"
        );
    }

    #[test]
    fn test_redact_repo_url_ssh() {
        assert_eq!(
            redact_repo_url("git@github.com:user/repo.git"),
            "git@github.com:user/repo.git"
        );
    }

    #[test]
    fn test_hash_path_deterministic() {
        let path = PathBuf::from("/home/user/doc.pdf");
        let h1 = hash_path(&path);
        let h2 = hash_path(&path);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn test_hash_path_different_paths_differ() {
        let h1 = hash_path(Path::new("/a/b"));
        let h2 = hash_path(Path::new("/c/d"));
        assert_ne!(h1, h2);
    }
}

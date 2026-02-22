//! Pure data types for git operations.

use serde::{Deserialize, Serialize};

/// Individual file status in the working tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    /// Relative file path.
    pub path: String,
    /// Status code: 'M' (modified), 'A' (added), 'D' (deleted), '?' (untracked), 'R' (renamed).
    pub status: char,
    /// Whether the file is staged for commit.
    pub staged: bool,
}

/// Git repository status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// Whether the directory is a git repository.
    pub is_repo: bool,
    /// Current branch name.
    pub branch: Option<String>,
    /// Whether the working tree is clean.
    pub is_clean: bool,
    /// Number of commits ahead of remote.
    pub ahead: u32,
    /// Number of commits behind remote.
    pub behind: u32,
    /// Modified files.
    pub modified_files: Vec<String>,
    /// Untracked files.
    pub untracked_files: Vec<String>,
    /// Per-file status information.
    pub files: Vec<FileStatus>,
}

/// Branch information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    /// Branch name.
    pub name: String,
    /// Whether this is the current branch.
    pub is_current: bool,
    /// Whether this is a remote branch.
    pub is_remote: bool,
}

/// Result of a git pull operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    /// Whether the pull succeeded.
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Number of files changed.
    pub files_changed: u32,
}

/// Result of a git commit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// Whether the commit succeeded.
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Commit hash if successful.
    pub commit_hash: Option<String>,
}

/// Status of a potential merge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStatus {
    /// Whether the merge can be fast-forwarded.
    pub can_fast_forward: bool,
    /// Number of commits ahead of remote.
    pub ahead: u32,
    /// Number of commits behind remote.
    pub behind: u32,
    /// Whether there are potential conflicts.
    pub has_conflicts: bool,
    /// List of files that would conflict.
    pub conflicting_files: Vec<String>,
}

/// Result of a git merge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeResult {
    /// Whether the merge succeeded.
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Number of files merged.
    pub merged_files: u32,
    /// List of files that have conflicts (if any).
    pub conflicting_files: Vec<String>,
}

/// Result of the git initialize operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Whether initialization completed successfully.
    pub initialized: bool,
    /// Whether a merge was performed.
    pub merged: bool,
    /// Status message.
    pub message: String,
    /// List of conflicting files (if merge had conflicts).
    pub conflicting_files: Vec<String>,
}

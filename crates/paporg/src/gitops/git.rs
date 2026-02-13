//! Git operations for GitOps configuration sync.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

use super::error::{GitOpsError, Result};
use super::progress::{GitOperationPhase, OperationProgress};
use super::resource::{GitAuthType, GitSettings};

/// Escapes a token for safe use in single-quoted shell strings.
/// Replaces single quotes with '\'' (end quote, escaped quote, start quote).
fn shell_escape_token(token: &str) -> String {
    token.replace('\'', "'\\''")
}

/// RAII guard for askpass script cleanup.
///
/// Automatically deletes the askpass script file when dropped, ensuring
/// sensitive tokens are not left on disk even if an error occurs.
pub struct AskpassCleanup {
    path: Option<PathBuf>,
}

impl AskpassCleanup {
    /// Creates a new cleanup guard for the given path.
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Creates an empty guard that does nothing on drop.
    fn empty() -> Self {
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

/// Formats a git error with both stdout and stderr for better debugging.
fn format_git_error(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    match (stderr.is_empty(), stdout.is_empty()) {
        (true, true) => format!(
            "Command failed with exit code {}",
            output.status.code().unwrap_or(-1)
        ),
        (true, false) => stdout,
        (false, true) => stderr,
        (false, false) => format!("{}\n{}", stderr, stdout),
    }
}

/// Git repository operations.
pub struct GitRepository {
    /// Path to the git repository.
    repo_path: PathBuf,
    /// Git settings from config.
    settings: GitSettings,
}

impl GitRepository {
    /// Creates a new git repository handle.
    pub fn new(repo_path: impl Into<PathBuf>, settings: GitSettings) -> Self {
        Self {
            repo_path: repo_path.into(),
            settings,
        }
    }

    /// Returns the repository path.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Checks if the directory is a git repository.
    pub fn is_git_repo(&self) -> bool {
        self.repo_path.join(".git").exists()
    }

    /// Checks if the repository has any commits.
    pub fn has_commits(&self) -> bool {
        if !self.is_git_repo() {
            return false;
        }

        // Try to get HEAD - if it fails, there are no commits
        self.run_git(&["rev-parse", "HEAD"])
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Checks out a remote branch, creating a local tracking branch.
    /// Used when initializing a repo with no local commits.
    pub fn checkout_remote_branch(&self, branch: &str) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let remote_ref = format!("origin/{}", branch);

        // Use checkout to create local branch tracking remote
        // -B forces creation even if branch exists, -f discards local changes
        let output = self.run_git(&["checkout", "-B", branch, &remote_ref])?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitOpsError::GitOperation(format_git_error(&output)))
        }
    }

    /// Gets the current branch name.
    pub fn current_branch(&self) -> Result<String> {
        let output = self.run_git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Gets the git status.
    pub fn status(&self) -> Result<GitStatus> {
        if !self.is_git_repo() {
            return Ok(GitStatus {
                is_repo: false,
                branch: None,
                is_clean: true,
                ahead: 0,
                behind: 0,
                modified_files: Vec::new(),
                untracked_files: Vec::new(),
                files: Vec::new(),
            });
        }

        let branch = self.current_branch().ok();

        // Get porcelain status
        let output = self.run_git(&["status", "--porcelain", "-b"])?;
        let status_text = String::from_utf8_lossy(&output.stdout);

        let mut modified_files = Vec::new();
        let mut untracked_files = Vec::new();
        let mut files = Vec::new();
        let mut ahead = 0;
        let mut behind = 0;

        for line in status_text.lines() {
            if line.starts_with("##") {
                // Branch line with tracking info
                if let Some(ahead_behind) = extract_ahead_behind(line) {
                    ahead = ahead_behind.0;
                    behind = ahead_behind.1;
                }
            } else if line.len() >= 3 {
                let index_status = line.chars().next().unwrap_or(' ');
                let worktree_status = line.chars().nth(1).unwrap_or(' ');
                let file_path = line[3..].trim().to_string();

                // Handle renamed files (format: "R  old -> new")
                let actual_path = if file_path.contains(" -> ") {
                    file_path
                        .split(" -> ")
                        .last()
                        .unwrap_or(&file_path)
                        .to_string()
                } else {
                    file_path.clone()
                };

                if line.starts_with("??") {
                    // Untracked file
                    untracked_files.push(actual_path.clone());
                    files.push(FileStatus {
                        path: actual_path,
                        status: '?',
                        staged: false,
                    });
                } else {
                    // Determine status and staged state
                    // Deleted in worktree takes priority (file no longer exists)
                    let (status, staged) = if worktree_status == 'D' {
                        ('D', false)
                    } else if index_status != ' ' && index_status != '?' {
                        // Staged change
                        (index_status, true)
                    } else if worktree_status != ' ' {
                        // Unstaged change
                        (worktree_status, false)
                    } else {
                        continue;
                    };

                    modified_files.push(actual_path.clone());
                    files.push(FileStatus {
                        path: actual_path,
                        status,
                        staged,
                    });
                }
            }
        }

        let is_clean = modified_files.is_empty() && untracked_files.is_empty();

        Ok(GitStatus {
            is_repo: true,
            branch,
            is_clean,
            ahead,
            behind,
            modified_files,
            untracked_files,
            files,
        })
    }

    /// Pulls changes from the remote repository.
    pub fn pull(&self) -> Result<PullResult> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        // Set up authentication if needed (cleanup guard ensures script is deleted)
        let (auth_env, _cleanup) = self.get_auth_env()?;

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_path).args([
            "pull",
            "--ff-only",
            "origin",
            &self.settings.branch,
        ]);

        // Add auth environment
        for (key, value) in &auth_env {
            cmd.env(key, value);
        }
        // Note: _cleanup is dropped after cmd completes, deleting askpass script

        let output = cmd
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let already_up_to_date = stdout.contains("Already up to date");

            Ok(PullResult {
                success: true,
                message: stdout.trim().to_string(),
                files_changed: if already_up_to_date {
                    0
                } else {
                    count_changed_files(&stdout)
                },
            })
        } else {
            Err(GitOpsError::GitOperation(format_git_error(&output)))
        }
    }

    /// Commits changes with the given message.
    pub fn commit(&self, message: &str, files: Option<&[&str]>) -> Result<CommitResult> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        // Add files
        if let Some(files) = files {
            for file in files {
                self.run_git(&["add", file])?;
            }
        } else {
            // Add all non-ignored files
            self.run_git(&["add", "."])?;
        }

        // Check if there are changes to commit
        let status = self.status()?;
        if status.is_clean {
            return Ok(CommitResult {
                success: true,
                message: "Nothing to commit".to_string(),
                commit_hash: None,
            });
        }

        // Commit
        let output = self.run_git(&["commit", "-m", message])?;

        if output.status.success() {
            // Get commit hash
            let hash_output = self.run_git(&["rev-parse", "--short", "HEAD"])?;
            let commit_hash = String::from_utf8_lossy(&hash_output.stdout)
                .trim()
                .to_string();

            Ok(CommitResult {
                success: true,
                message: String::from_utf8_lossy(&output.stdout).trim().to_string(),
                commit_hash: Some(commit_hash),
            })
        } else {
            Err(GitOpsError::GitOperation(format_git_error(&output)))
        }
    }

    /// Pushes commits to the remote repository.
    pub fn push(&self) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let (auth_env, _cleanup) = self.get_auth_env()?;

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["push", "origin", &self.settings.branch]);

        for (key, value) in &auth_env {
            cmd.env(key, value);
        }
        // _cleanup dropped after cmd completes

        let output = cmd
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitOpsError::GitOperation(format_git_error(&output)))
        }
    }

    /// Commits and pushes in one operation.
    pub fn commit_and_push(&self, message: &str, files: Option<&[&str]>) -> Result<CommitResult> {
        let commit_result = self.commit(message, files)?;

        if commit_result.commit_hash.is_some() {
            self.push()?;
        }

        Ok(commit_result)
    }

    /// Pulls changes with progress reporting.
    pub async fn pull_with_progress(&self, progress: &OperationProgress) -> Result<PullResult> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        progress.phase(GitOperationPhase::Pulling, "Starting pull...");

        let (auth_env, _cleanup) = self.get_auth_env()?;

        let mut cmd = TokioCommand::new("git");
        cmd.current_dir(&self.repo_path)
            .args([
                "pull",
                "--ff-only",
                "--progress",
                "origin",
                &self.settings.branch,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &auth_env {
            cmd.env(key, value);
        }
        // _cleanup kept alive until end of function

        let mut child = cmd
            .spawn()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        // Stream stderr (where git progress goes)
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                progress.raw_output(&line);
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let already_up_to_date = stdout.contains("Already up to date");

            progress.completed(&format!(
                "Pull completed{}",
                if already_up_to_date {
                    " - already up to date"
                } else {
                    ""
                }
            ));

            Ok(PullResult {
                success: true,
                message: stdout.trim().to_string(),
                files_changed: if already_up_to_date {
                    0
                } else {
                    count_changed_files(&stdout)
                },
            })
        } else {
            let error = format_git_error(&output);
            progress.failed(&error);
            Err(GitOpsError::GitOperation(error))
        }
    }

    /// Commits changes with progress reporting.
    pub async fn commit_with_progress(
        &self,
        message: &str,
        files: Option<&[&str]>,
        progress: &OperationProgress,
    ) -> Result<CommitResult> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        progress.phase(GitOperationPhase::StagingFiles, "Staging files...");

        // Add files
        if let Some(files) = files {
            for file in files {
                self.run_git(&["add", file])?;
            }
        } else {
            self.run_git(&["add", "."])?;
        }

        // Check if there are changes to commit
        let status = self.status()?;
        if status.is_clean {
            progress.completed("Nothing to commit");
            return Ok(CommitResult {
                success: true,
                message: "Nothing to commit".to_string(),
                commit_hash: None,
            });
        }

        progress.phase(GitOperationPhase::Committing, "Creating commit...");

        // Commit
        let output = self.run_git(&["commit", "-m", message])?;

        if output.status.success() {
            // Get commit hash
            let hash_output = self.run_git(&["rev-parse", "--short", "HEAD"])?;
            let commit_hash = String::from_utf8_lossy(&hash_output.stdout)
                .trim()
                .to_string();

            progress.completed(&format!("Committed: {}", commit_hash));

            Ok(CommitResult {
                success: true,
                message: String::from_utf8_lossy(&output.stdout).trim().to_string(),
                commit_hash: Some(commit_hash),
            })
        } else {
            let error = format_git_error(&output);
            progress.failed(&error);
            Err(GitOpsError::GitOperation(error))
        }
    }

    /// Pushes commits with progress reporting.
    pub async fn push_with_progress(&self, progress: &OperationProgress) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        progress.phase(GitOperationPhase::Pushing, "Pushing to remote...");

        let (auth_env, _cleanup) = self.get_auth_env()?;

        let mut cmd = TokioCommand::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["push", "--progress", "origin", &self.settings.branch])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &auth_env {
            cmd.env(key, value);
        }
        // _cleanup kept alive until end of function

        let mut child = cmd
            .spawn()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        // Stream stderr (where git progress goes)
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                progress.raw_output(&line);
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            progress.completed("Push completed");
            Ok(())
        } else {
            let error = format_git_error(&output);
            progress.failed(&error);
            Err(GitOpsError::GitOperation(error))
        }
    }

    /// Commits and pushes with progress reporting.
    pub async fn commit_and_push_with_progress(
        &self,
        message: &str,
        files: Option<&[&str]>,
        progress: &OperationProgress,
    ) -> Result<CommitResult> {
        let commit_result = self.commit_with_progress(message, files, progress).await?;

        if commit_result.commit_hash.is_some() {
            self.push_with_progress(progress).await?;
        }

        Ok(commit_result)
    }

    /// Fetches from remote with progress reporting.
    pub async fn fetch_with_progress(
        &self,
        branch: &str,
        progress: &OperationProgress,
    ) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        progress.phase(GitOperationPhase::Fetching, "Fetching from remote...");

        let (auth_env, _cleanup) = self.get_auth_env()?;

        let mut cmd = TokioCommand::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["fetch", "--progress", "origin", branch])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &auth_env {
            cmd.env(key, value);
        }
        // _cleanup kept alive until end of function

        let mut child = cmd
            .spawn()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        // Stream stderr (where git progress goes)
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                progress.raw_output(&line);
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            progress.completed("Fetch completed");
            Ok(())
        } else {
            let error = format_git_error(&output);
            progress.failed(&error);
            Err(GitOpsError::GitOperation(error))
        }
    }

    /// Initializes a git repository if one doesn't exist.
    pub fn init(&self) -> Result<()> {
        if self.is_git_repo() {
            return Ok(());
        }

        let output = self.run_git(&["init"])?;

        if !output.status.success() {
            return Err(GitOpsError::GitOperation(format_git_error(&output)));
        }

        // Configure git user for commits
        let _ = self.run_git(&["config", "user.email", &self.settings.user_email]);
        let _ = self.run_git(&["config", "user.name", &self.settings.user_name]);

        Ok(())
    }

    /// Clones a repository.
    pub fn clone(url: &str, target_path: &Path, branch: &str) -> Result<Self> {
        let mut cmd = Command::new("git");
        cmd.args(["clone", "--branch", branch, "--single-branch", url])
            .arg(target_path);

        let output = cmd
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            Ok(Self {
                repo_path: target_path.to_path_buf(),
                settings: GitSettings {
                    enabled: true,
                    repository: url.to_string(),
                    branch: branch.to_string(),
                    ..Default::default()
                },
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(GitOpsError::GitOperation(stderr.trim().to_string()))
        }
    }

    /// Sets the remote URL.
    pub fn set_remote(&self, url: &str) -> Result<()> {
        // Check if remote exists by checking the exit status
        let check = self.run_git(&["remote", "get-url", "origin"]);

        let remote_exists = check.map(|output| output.status.success()).unwrap_or(false);

        if remote_exists {
            // Update existing remote
            let output = self.run_git(&["remote", "set-url", "origin", url])?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitOpsError::GitOperation(stderr.trim().to_string()));
            }
        } else {
            // Add new remote
            let output = self.run_git(&["remote", "add", "origin", url])?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitOpsError::GitOperation(stderr.trim().to_string()));
            }
        }

        Ok(())
    }

    /// Lists all branches (local and remote).
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let mut branches = Vec::new();
        let current = self.current_branch().ok();

        // Get local branches
        let output = self.run_git(&["branch", "--list"])?;
        let output_text = String::from_utf8_lossy(&output.stdout);

        for line in output_text.lines() {
            let is_current = line.starts_with('*');
            let name = line.trim_start_matches('*').trim().to_string();
            if !name.is_empty() && !name.starts_with("(") {
                branches.push(BranchInfo {
                    name,
                    is_current,
                    is_remote: false,
                });
            }
        }

        // Get remote branches
        let output = self.run_git(&["branch", "-r", "--list"])?;
        let output_text = String::from_utf8_lossy(&output.stdout);

        for line in output_text.lines() {
            let name = line.trim().to_string();
            // Skip HEAD pointer and empty lines
            if !name.is_empty() && !name.contains("->") {
                // Remove origin/ prefix for display
                let display_name = name.strip_prefix("origin/").unwrap_or(&name).to_string();
                // Check if we already have a local branch with this name
                if !branches
                    .iter()
                    .any(|b| !b.is_remote && b.name == display_name)
                {
                    branches.push(BranchInfo {
                        name: display_name,
                        is_current: current.as_ref() == Some(&name),
                        is_remote: true,
                    });
                }
            }
        }

        Ok(branches)
    }

    /// Checks out a branch.
    pub fn checkout(&self, branch: &str) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        // First try regular checkout (works for existing local branches)
        let output = self.run_git(&["checkout", branch])?;

        if output.status.success() {
            return Ok(());
        }

        // If failed, try creating local branch tracking remote
        // This handles the case of checking out a remote branch for the first time
        let remote_ref = format!("origin/{}", branch);
        let output = self.run_git(&["checkout", "-b", branch, &remote_ref])?;

        if output.status.success() {
            return Ok(());
        }

        // Return the error from the tracking branch attempt
        Err(GitOpsError::GitOperation(format_git_error(&output)))
    }

    /// Creates a new branch and optionally checks it out.
    pub fn create_branch(&self, name: &str, checkout: bool) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        if checkout {
            let output = self.run_git(&["checkout", "-b", name])?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitOpsError::GitOperation(stderr.trim().to_string()));
            }
        } else {
            let output = self.run_git(&["branch", name])?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitOpsError::GitOperation(stderr.trim().to_string()));
            }
        }

        Ok(())
    }

    /// Fetches from remote without merging.
    pub fn fetch(&self, branch: &str) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let (auth_env, _cleanup) = self.get_auth_env()?;

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["fetch", "origin", branch]);

        for (key, value) in &auth_env {
            cmd.env(key, value);
        }
        // _cleanup dropped after cmd completes

        let output = cmd
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitOpsError::GitOperation(format_git_error(&output)))
        }
    }

    /// Checks merge status before attempting merge.
    pub fn merge_status(&self, branch: &str) -> Result<MergeStatus> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        // Get ahead/behind counts
        let remote_ref = format!("origin/{}", branch);
        let output = self.run_git(&[
            "rev-list",
            "--left-right",
            "--count",
            &format!("HEAD...{}", remote_ref),
        ]);

        let (ahead, behind) = if let Ok(output) = output {
            if output.status.success() {
                let counts = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = counts.trim().split('\t').collect();
                if parts.len() == 2 {
                    let ahead = parts[0].parse().unwrap_or(0);
                    let behind = parts[1].parse().unwrap_or(0);
                    (ahead, behind)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };

        // Check if can fast-forward (no local commits ahead)
        let can_fast_forward = ahead == 0;

        // Check for potential conflicts by doing a dry-run merge
        let mut has_conflicts = false;
        let mut conflicting_files = Vec::new();

        if behind > 0 {
            // Try merge with --no-commit --no-ff to see if it would succeed
            let merge_check = self.run_git(&["merge", "--no-commit", "--no-ff", &remote_ref]);

            if let Ok(output) = merge_check {
                if !output.status.success() {
                    has_conflicts = true;
                    // Parse conflict output
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);

                    // Look for conflict markers in output
                    for line in stderr.lines().chain(stdout.lines()) {
                        if line.contains("CONFLICT") {
                            // Extract filename from lines like "CONFLICT (content): Merge conflict in filename"
                            if let Some(file) = line.split("Merge conflict in ").last() {
                                conflicting_files.push(file.trim().to_string());
                            } else if let Some(file) = line.split("CONFLICT (").nth(1) {
                                // Try other conflict formats
                                if let Some(file) = file.split("):").nth(1) {
                                    let file = file.trim();
                                    if !file.is_empty() {
                                        conflicting_files.push(file.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                // Abort the merge attempt
                let _ = self.run_git(&["merge", "--abort"]);
            }
        }

        Ok(MergeStatus {
            can_fast_forward,
            ahead,
            behind,
            has_conflicts,
            conflicting_files,
        })
    }

    /// Attempts to merge remote branch into current.
    pub fn merge(&self, branch: &str) -> Result<MergeResult> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let remote_ref = format!("origin/{}", branch);
        let output = self.run_git(&["merge", &remote_ref])?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let merged_files = count_changed_files(&stdout);

            Ok(MergeResult {
                success: true,
                message: stdout.trim().to_string(),
                merged_files,
                conflicting_files: Vec::new(),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check if it's a conflict
            let mut conflicting_files = Vec::new();
            for line in stderr.lines().chain(stdout.lines()) {
                if line.contains("CONFLICT") {
                    if let Some(file) = line.split("Merge conflict in ").last() {
                        conflicting_files.push(file.trim().to_string());
                    }
                }
            }

            if !conflicting_files.is_empty() {
                Ok(MergeResult {
                    success: false,
                    message: "Merge conflicts detected".to_string(),
                    merged_files: 0,
                    conflicting_files,
                })
            } else {
                Err(GitOpsError::GitOperation(stderr.trim().to_string()))
            }
        }
    }

    /// Aborts an in-progress merge.
    pub fn merge_abort(&self) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let output = self.run_git(&["merge", "--abort"])?;

        if output.status.success() {
            Ok(())
        } else {
            // It's okay if there's no merge in progress
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no merge") || stderr.contains("not merging") {
                Ok(())
            } else {
                Err(GitOpsError::GitOperation(stderr.trim().to_string()))
            }
        }
    }

    /// Runs a git command in the repository directory.
    fn run_git(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(args)
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        Ok(output)
    }

    /// Gets authentication environment variables and cleanup guard.
    ///
    /// Returns a tuple of (env vars, cleanup guard). The cleanup guard MUST be
    /// kept alive until the git operation completes - dropping it will delete
    /// the askpass script file.
    fn get_auth_env(&self) -> Result<(Vec<(String, String)>, AskpassCleanup)> {
        use secrecy::ExposeSecret;

        let mut env = Vec::new();

        match self.settings.auth.auth_type {
            GitAuthType::None => {
                return Ok((env, AskpassCleanup::empty()));
            }
            GitAuthType::Token => {
                // Get token from configured sources (direct, file, or env var)
                let env_var = if self.settings.auth.token_env_var.is_empty() {
                    None
                } else {
                    Some(self.settings.auth.token_env_var.as_str())
                };

                let token = crate::secrets::resolve_secret(
                    self.settings.auth.token_insecure.as_deref(),
                    self.settings.auth.token_file.as_deref(),
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
                    // Windows batch script that echoes the token
                    let script = format!("@echo off\r\necho {}\r\n", escaped_token);
                    (path, script)
                };

                // Write with restrictive permissions from the start on Unix
                // Create directly executable (0o700) since we're using create_new which
                // prevents race conditions with existing files
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
                    // On Windows, write to file and set restrictive permissions if possible
                    std::fs::write(&askpass_path, &askpass_script)?;
                    // Note: Windows file permissions are more complex; the file is at least
                    // in the user's temp directory which has appropriate ACLs by default
                }

                // Create cleanup guard before adding to env
                let cleanup = AskpassCleanup::new(askpass_path.clone());

                // Use to_str() instead of to_string_lossy() to ensure valid UTF-8 path
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

                return Ok((env, cleanup));
            }
            GitAuthType::SshKey => {
                let key_path = if self.settings.auth.ssh_key_path.is_empty() {
                    // Try default paths
                    let home = std::env::var("HOME").unwrap_or_default();
                    format!("{}/.ssh/id_ed25519", home)
                } else {
                    // Expand ~ to HOME (handles both ~/path and standalone ~)
                    let path = &self.settings.auth.ssh_key_path;
                    if path == "~" {
                        std::env::var("HOME").unwrap_or_default()
                    } else if path.starts_with("~/") {
                        let home = std::env::var("HOME").unwrap_or_default();
                        format!("{}{}", home, &path[1..])
                    } else {
                        path.clone()
                    }
                };

                if !Path::new(&key_path).exists() {
                    return Err(GitOpsError::GitAuthFailed(format!(
                        "SSH key file not found: {}",
                        key_path
                    )));
                }

                // Use StrictHostKeyChecking=accept-new to accept new hosts but reject changed keys
                // This is safer than 'no' while still allowing first-time connections
                env.push((
                    "GIT_SSH_COMMAND".to_string(),
                    format!("ssh -i {} -o StrictHostKeyChecking=accept-new", key_path),
                ));
            }
        }

        Ok((env, AskpassCleanup::empty()))
    }
}

/// Individual file status in the working tree.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Git sync manager for periodic synchronization.
pub struct GitSyncManager {
    repo: GitRepository,
    interval: Duration,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl GitSyncManager {
    /// Creates a new sync manager.
    pub fn new(repo: GitRepository) -> Self {
        let interval = Duration::from_secs(repo.settings.sync_interval);
        Self {
            repo,
            interval,
            shutdown: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Starts periodic sync in a background thread.
    pub fn start(&self) -> std::thread::JoinHandle<()> {
        let repo_path = self.repo.repo_path.clone();
        let settings = self.repo.settings.clone();
        let interval = self.interval;
        let shutdown = std::sync::Arc::clone(&self.shutdown);

        std::thread::spawn(move || {
            let repo = GitRepository::new(repo_path, settings);

            loop {
                if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                // Sleep in small intervals to check shutdown
                let start = std::time::Instant::now();
                while start.elapsed() < interval {
                    if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }

                // Try to pull
                match repo.pull() {
                    Ok(result) => {
                        if result.files_changed > 0 {
                            log::info!("Git sync: pulled {} files", result.files_changed);
                        }
                    }
                    Err(e) => {
                        log::error!("Git sync failed: {}", e);
                    }
                }
            }
        })
    }

    /// Signals the sync manager to stop.
    pub fn stop(&self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Extracts ahead/behind counts from git status branch line.
fn extract_ahead_behind(line: &str) -> Option<(u32, u32)> {
    let mut ahead = 0;
    let mut behind = 0;

    if let Some(bracket_start) = line.find('[') {
        if let Some(bracket_end) = line.find(']') {
            let info = &line[bracket_start + 1..bracket_end];

            for part in info.split(',') {
                let part = part.trim();
                if let Some(n) = part.strip_prefix("ahead ") {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix("behind ") {
                    behind = n.parse().unwrap_or(0);
                }
            }
        }
    }

    Some((ahead, behind))
}

/// Counts changed files from git pull output.
fn count_changed_files(output: &str) -> u32 {
    // Look for patterns like "X files changed" or "X insertions" or "X deletions"
    for line in output.lines() {
        if line.contains("file") && line.contains("changed") {
            // Try to parse the first number
            for word in line.split_whitespace() {
                if let Ok(n) = word.parse::<u32>() {
                    return n;
                }
            }
        }
    }
    0
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_git_repo_false() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());
        assert!(!repo.is_git_repo());
    }

    #[test]
    fn test_init_and_is_git_repo() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());

        repo.init().unwrap();
        assert!(repo.is_git_repo());
    }

    #[test]
    fn test_status_not_repo() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());

        let status = repo.status().unwrap();
        assert!(!status.is_repo);
        assert!(status.is_clean);
    }

    #[test]
    fn test_status_clean_repo() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());

        repo.init().unwrap();

        let status = repo.status().unwrap();
        assert!(status.is_repo);
        assert!(status.is_clean);
    }

    #[test]
    fn test_status_with_changes() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());

        repo.init().unwrap();

        // Create a file
        std::fs::write(dir.path().join("test.yaml"), "test: true").unwrap();

        let status = repo.status().unwrap();
        assert!(status.is_repo);
        assert!(!status.is_clean);
        assert!(status.untracked_files.contains(&"test.yaml".to_string()));
    }

    #[test]
    fn test_commit_no_changes() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());

        repo.init().unwrap();

        // Configure git user for commits
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["config", "user.email", "test@test.com"])
            .output();
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["config", "user.name", "Test"])
            .output();

        let result = repo.commit("Test commit", None).unwrap();
        assert!(result.success);
        assert!(result.message.contains("Nothing to commit"));
        assert!(result.commit_hash.is_none());
    }

    #[test]
    fn test_extract_ahead_behind() {
        assert_eq!(
            extract_ahead_behind("## main...origin/main [ahead 2]"),
            Some((2, 0))
        );
        assert_eq!(
            extract_ahead_behind("## main...origin/main [behind 3]"),
            Some((0, 3))
        );
        assert_eq!(
            extract_ahead_behind("## main...origin/main [ahead 1, behind 2]"),
            Some((1, 2))
        );
        assert_eq!(extract_ahead_behind("## main"), Some((0, 0)));
    }

    #[test]
    fn test_count_changed_files() {
        assert_eq!(count_changed_files("3 files changed, 10 insertions(+)"), 3);
        assert_eq!(count_changed_files("1 file changed, 1 insertion(+)"), 1);
        assert_eq!(count_changed_files("Already up to date."), 0);
    }

    #[test]
    fn test_git_status_serialization() {
        let status = GitStatus {
            is_repo: true,
            branch: Some("main".to_string()),
            is_clean: false,
            ahead: 2,
            behind: 1,
            modified_files: vec!["test.yaml".to_string()],
            untracked_files: vec![],
            files: vec![FileStatus {
                path: "test.yaml".to_string(),
                status: 'M',
                staged: false,
            }],
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"isRepo\":true"));
        assert!(json.contains("\"isClean\":false"));
        assert!(json.contains("\"ahead\":2"));
        assert!(json.contains("\"files\""));
    }

    #[test]
    fn test_pull_result_serialization() {
        let result = PullResult {
            success: true,
            message: "Already up to date.".to_string(),
            files_changed: 0,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"filesChanged\":0"));
    }

    #[test]
    fn test_commit_result_serialization() {
        let result = CommitResult {
            success: true,
            message: "Test commit".to_string(),
            commit_hash: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"commitHash\":\"abc123\""));
    }
}

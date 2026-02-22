//! Git repository operations.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

use super::auth::{build_auth_env, AuthEnv};
use super::parse::{count_changed_files, extract_ahead_behind, format_git_error};
use super::types::*;
use crate::gitops::error::{GitOpsError, Result};
use crate::gitops::progress::{GitOperationPhase, OperationProgress};
use crate::gitops::resource::GitSettings;

/// Git repository operations.
pub struct GitRepository {
    /// Path to the git repository.
    repo_path: PathBuf,
    /// Git settings from config.
    pub(crate) settings: GitSettings,
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

    /// Returns the configured branch name.
    pub fn branch(&self) -> &str {
        &self.settings.branch
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

        self.run_git(&["rev-parse", "HEAD"])
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Checks out a remote branch, creating a local tracking branch.
    pub fn checkout_remote_branch(&self, branch: &str) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let remote_ref = format!("origin/{}", branch);
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

        let output = self.run_git(&["status", "--porcelain", "-b"])?;
        let status_text = String::from_utf8_lossy(&output.stdout);

        let mut modified_files = Vec::new();
        let mut untracked_files = Vec::new();
        let mut files = Vec::new();
        let mut ahead = 0;
        let mut behind = 0;

        for line in status_text.lines() {
            if line.starts_with("##") {
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
                    untracked_files.push(actual_path.clone());
                    files.push(FileStatus {
                        path: actual_path,
                        status: '?',
                        staged: false,
                    });
                } else {
                    let (status, staged) = if worktree_status == 'D' {
                        ('D', false)
                    } else if index_status != ' ' && index_status != '?' {
                        (index_status, true)
                    } else if worktree_status != ' ' {
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

    /// Sets the remote URL.
    pub fn set_remote(&self, url: &str) -> Result<()> {
        let check = self.run_git(&["remote", "get-url", "origin"]);
        let remote_exists = check.map(|output| output.status.success()).unwrap_or(false);

        if remote_exists {
            let output = self.run_git(&["remote", "set-url", "origin", url])?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitOpsError::GitOperation(stderr.trim().to_string()));
            }
        } else {
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
            if !name.is_empty() && !name.contains("->") {
                let display_name = name.strip_prefix("origin/").unwrap_or(&name).to_string();
                if !branches
                    .iter()
                    .any(|b| !b.is_remote && b.name == display_name)
                {
                    branches.push(BranchInfo {
                        name: display_name,
                        is_current: false,
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

        let output = self.run_git(&["checkout", branch])?;

        if output.status.success() {
            return Ok(());
        }

        // If failed, try creating local branch tracking remote
        let remote_ref = format!("origin/{}", branch);
        let output = self.run_git(&["checkout", "-b", branch, &remote_ref])?;

        if output.status.success() {
            return Ok(());
        }

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

    /// Checks merge status before attempting merge.
    pub fn merge_status(&self, branch: &str) -> Result<MergeStatus> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

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

        let can_fast_forward = ahead == 0;

        let mut has_conflicts = false;
        let mut conflicting_files = Vec::new();

        if behind > 0 {
            // Use git merge-tree (plumbing) for non-destructive conflict detection
            let merge_base = self.run_git(&["merge-base", "HEAD", &remote_ref]);
            if let Ok(base_output) = merge_base {
                if base_output.status.success() {
                    let base = String::from_utf8_lossy(&base_output.stdout)
                        .trim()
                        .to_string();
                    let merge_tree =
                        self.run_git(&["merge-tree", &base, "HEAD", &remote_ref]);
                    if let Ok(tree_output) = merge_tree {
                        let tree_text = String::from_utf8_lossy(&tree_output.stdout);
                        for line in tree_text.lines() {
                            if line.contains("changed in both") {
                                has_conflicts = true;
                                if let Some(file) = line.split_whitespace().last() {
                                    conflicting_files.push(file.to_string());
                                }
                            }
                        }
                    }
                }
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no merge") || stderr.contains("not merging") {
                Ok(())
            } else {
                Err(GitOpsError::GitOperation(stderr.trim().to_string()))
            }
        }
    }

    // ========================================================================
    // Async methods with progress reporting
    // ========================================================================

    /// Pulls changes with progress reporting.
    pub async fn pull_with_progress(&self, progress: &OperationProgress) -> Result<PullResult> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        progress.phase(GitOperationPhase::Pulling, "Starting pull...");

        let auth = self.get_auth_env()?;

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

        for (key, value) in &auth.env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        let stderr_pipe = child.stderr.take();
        let stdout_pipe = child.stdout.take();

        let stderr_task = async {
            if let Some(stderr) = stderr_pipe {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    progress.raw_output(&line);
                }
            }
        };

        let stdout_task = async {
            let mut collected = Vec::new();
            if let Some(stdout) = stdout_pipe {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    collected.push(line);
                }
            }
            collected
        };

        let ((), stdout_lines) = tokio::join!(stderr_task, stdout_task);
        let stdout_text = stdout_lines.join("\n");

        let status = child
            .wait()
            .await
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        // Drop auth env (cleanup guard) after command completes
        drop(auth);

        if status.success() {
            let already_up_to_date = stdout_text.contains("Already up to date");

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
                message: stdout_text.trim().to_string(),
                files_changed: if already_up_to_date {
                    0
                } else {
                    count_changed_files(&stdout_text)
                },
            })
        } else {
            let error = format!("Pull failed with exit code {}", status.code().unwrap_or(-1));
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

        if let Some(files) = files {
            for file in files {
                self.run_git(&["add", file])?;
            }
        } else {
            self.run_git(&["add", "."])?;
        }

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

        let output = self.run_git(&["commit", "-m", message])?;

        if output.status.success() {
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

        let auth = self.get_auth_env()?;

        let mut cmd = TokioCommand::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["push", "--progress", "origin", &self.settings.branch])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &auth.env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        let stderr_pipe = child.stderr.take();
        let stdout_pipe = child.stdout.take();

        let stderr_task = async {
            if let Some(stderr) = stderr_pipe {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    progress.raw_output(&line);
                }
            }
        };

        let stdout_task = async {
            if let Some(stdout) = stdout_pipe {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(_line)) = lines.next_line().await {
                    // Drain stdout to prevent pipe buffer from filling
                }
            }
        };

        tokio::join!(stderr_task, stdout_task);

        let status = child
            .wait()
            .await
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        drop(auth);

        if status.success() {
            progress.completed("Push completed");
            Ok(())
        } else {
            let error = format!("Push failed with exit code {}", status.code().unwrap_or(-1));
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

        let auth = self.get_auth_env()?;

        let mut cmd = TokioCommand::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["fetch", "--progress", "origin", branch])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &auth.env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        let stderr_pipe = child.stderr.take();
        let stdout_pipe = child.stdout.take();

        let stderr_task = async {
            if let Some(stderr) = stderr_pipe {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    progress.raw_output(&line);
                }
            }
        };

        let stdout_task = async {
            if let Some(stdout) = stdout_pipe {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(_line)) = lines.next_line().await {
                    // Drain stdout to prevent pipe buffer from filling
                }
            }
        };

        tokio::join!(stderr_task, stdout_task);

        let status = child
            .wait()
            .await
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        drop(auth);

        if status.success() {
            progress.completed("Fetch completed");
            Ok(())
        } else {
            let error = format!("Fetch failed with exit code {}", status.code().unwrap_or(-1));
            progress.failed(&error);
            Err(GitOpsError::GitOperation(error))
        }
    }

    /// Fetches from remote without merging (sync).
    pub fn fetch(&self, branch: &str) -> Result<()> {
        if !self.is_git_repo() {
            return Err(GitOpsError::GitNotInitialized);
        }

        let auth = self.get_auth_env()?;

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["fetch", "origin", branch]);

        for (key, value) in &auth.env_vars {
            cmd.env(key, value);
        }

        let output = cmd
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        drop(auth);

        if output.status.success() {
            Ok(())
        } else {
            Err(GitOpsError::GitOperation(format_git_error(&output)))
        }
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Runs a git command in the repository directory.
    fn run_git(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(args)
            .output()
            .map_err(|e| GitOpsError::GitOperation(e.to_string()))?;

        Ok(output)
    }

    /// Gets authentication environment for git commands.
    fn get_auth_env(&self) -> Result<AuthEnv> {
        build_auth_env(&self.settings.auth)
    }
}

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

        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["config", "user.email", "test@test.com"])
            .output();
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["config", "user.name", "Test"])
            .output();

        // Use commit_with_progress through a runtime
        let rt = tokio::runtime::Runtime::new().unwrap();
        let progress = crate::broadcast::GitProgressBroadcaster::default();
        let op = progress.start_operation(crate::gitops::progress::GitOperationType::Commit);
        let result = rt
            .block_on(repo.commit_with_progress("Test commit", None, &op))
            .unwrap();
        assert!(result.success);
        assert!(result.message.contains("Nothing to commit"));
        assert!(result.commit_hash.is_none());
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

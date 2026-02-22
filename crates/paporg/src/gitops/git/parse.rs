//! Git output parsing helpers.

use std::process::Output;

/// Formats a git error with both stdout and stderr for better debugging.
pub fn format_git_error(output: &Output) -> String {
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

/// Extracts ahead/behind counts from git status branch line.
/// Returns `None` when no bracket info is present.
pub fn extract_ahead_behind(line: &str) -> Option<(u32, u32)> {
    let bracket_start = line.find('[')?;
    let bracket_end = line.find(']')?;
    if bracket_end <= bracket_start {
        return None;
    }

    let info = &line[bracket_start + 1..bracket_end];
    let mut ahead = 0;
    let mut behind = 0;

    for part in info.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        }
    }

    Some((ahead, behind))
}

/// Counts changed files from git pull output.
pub fn count_changed_files(output: &str) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(extract_ahead_behind("## main"), None);
    }

    #[test]
    fn test_count_changed_files() {
        assert_eq!(count_changed_files("3 files changed, 10 insertions(+)"), 3);
        assert_eq!(count_changed_files("1 file changed, 1 insertion(+)"), 1);
        assert_eq!(count_changed_files("Already up to date."), 0);
    }

    #[cfg(unix)]
    mod unix_tests {
        use super::*;
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        fn make_output(status_code: i32, stdout: &[u8], stderr: &[u8]) -> Output {
            Output {
                status: ExitStatus::from_raw(status_code << 8),
                stdout: stdout.to_vec(),
                stderr: stderr.to_vec(),
            }
        }

        #[test]
        fn test_format_git_error_empty_output() {
            let output = make_output(1, b"", b"");
            assert_eq!(format_git_error(&output), "Command failed with exit code 1");
        }

        #[test]
        fn test_format_git_error_stderr_only() {
            let output = make_output(1, b"", b"fatal: not a git repository");
            assert_eq!(
                format_git_error(&output),
                "fatal: not a git repository"
            );
        }

        #[test]
        fn test_format_git_error_both() {
            let output = make_output(1, b"some output", b"some error");
            assert_eq!(format_git_error(&output), "some error\nsome output");
        }
    }
}

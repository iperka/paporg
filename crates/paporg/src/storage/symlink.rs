use std::path::{Path, PathBuf};

use crate::error::StorageError;

pub struct SymlinkManager {
    output_directory: PathBuf,
}

impl SymlinkManager {
    pub fn new<P: AsRef<Path>>(output_directory: P) -> Self {
        Self {
            output_directory: output_directory.as_ref().to_path_buf(),
        }
    }

    pub fn create_symlink(
        &self,
        target_file: &Path,
        symlink_directory: &str,
    ) -> Result<PathBuf, StorageError> {
        let symlink_dir = self.output_directory.join(symlink_directory);

        // Ensure symlink directory exists
        if !symlink_dir.exists() {
            std::fs::create_dir_all(&symlink_dir).map_err(|e| StorageError::CreateDirectory {
                path: symlink_dir.clone(),
                source: e,
            })?;
        }

        // Get filename from target
        let filename = target_file
            .file_name()
            .ok_or_else(|| StorageError::CreateSymlink {
                link: symlink_dir.clone(),
                target: target_file.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid target filename",
                ),
            })?;

        let symlink_path = symlink_dir.join(filename);

        // Calculate relative path from symlink to target
        let relative_target = self.calculate_relative_path(&symlink_path, target_file)?;

        // Handle existing symlink - try to remove it atomically
        // This is inherently racy but we handle the error case below
        if std::fs::symlink_metadata(&symlink_path).is_ok() {
            // Try to remove existing file/symlink
            if let Err(e) = std::fs::remove_file(&symlink_path) {
                // Only log if it's not a "file not found" error (another process may have removed it)
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::debug!(
                        "Could not remove existing symlink {:?}: {}",
                        symlink_path,
                        e
                    );
                }
            }
        }

        // Create symlink
        #[cfg(unix)]
        std::os::unix::fs::symlink(&relative_target, &symlink_path).map_err(|e| {
            StorageError::CreateSymlink {
                link: symlink_path.clone(),
                target: target_file.to_path_buf(),
                source: e,
            }
        })?;

        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&relative_target, &symlink_path).map_err(|e| {
            StorageError::CreateSymlink {
                link: symlink_path.clone(),
                target: target_file.to_path_buf(),
                source: e,
            }
        })?;

        Ok(symlink_path)
    }

    fn calculate_relative_path(&self, from: &Path, to: &Path) -> Result<PathBuf, StorageError> {
        // Get the directory containing the symlink
        let from_dir = from.parent().unwrap_or(Path::new("."));

        // Make both paths absolute for comparison
        let from_abs = if from_dir.is_absolute() {
            from_dir.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(from_dir)
        };

        let to_abs = if to.is_absolute() {
            to.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(to)
        };

        // Canonicalize paths (resolve any .. or .)
        let from_canonical = from_abs.canonicalize().unwrap_or(from_abs);
        let to_canonical = to_abs.canonicalize().unwrap_or(to_abs);

        // Find common ancestor
        let from_components: Vec<_> = from_canonical.components().collect();
        let to_components: Vec<_> = to_canonical.components().collect();

        let mut common_length = 0;
        for (i, (a, b)) in from_components.iter().zip(to_components.iter()).enumerate() {
            if a == b {
                common_length = i + 1;
            } else {
                break;
            }
        }

        // Build relative path
        let mut relative = PathBuf::new();

        // Add ".." for each remaining component in from_path
        for _ in common_length..from_components.len() {
            relative.push("..");
        }

        // Add remaining components from to_path
        for component in &to_components[common_length..] {
            relative.push(component);
        }

        Ok(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create a target file
        let target_dir = temp_dir.path().join("2026/invoices");
        std::fs::create_dir_all(&target_dir).unwrap();
        let target_file = target_dir.join("invoice.pdf");
        std::fs::write(&target_file, b"Test content").unwrap();

        // Create symlink
        let symlink_path = manager.create_symlink(&target_file, "taxes/2026").unwrap();

        assert!(symlink_path.exists());
        assert!(symlink_path.is_symlink());

        // Read through symlink should work
        let content = std::fs::read(&symlink_path).unwrap();
        assert_eq!(content, b"Test content");
    }

    #[test]
    fn test_relative_path_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create directories
        let dir1 = temp_dir.path().join("a/b/c");
        let dir2 = temp_dir.path().join("a/d/e");
        std::fs::create_dir_all(&dir1).unwrap();
        std::fs::create_dir_all(&dir2).unwrap();

        let from = dir1.join("link");
        let to = dir2.join("target.txt");
        std::fs::write(&to, "test").unwrap();

        let relative = manager.calculate_relative_path(&from, &to).unwrap();

        // Should be something like "../../d/e/target.txt"
        assert!(relative.to_string_lossy().contains(".."));
    }

    #[test]
    fn test_overwrite_existing_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create two target files
        let target1 = temp_dir.path().join("target1.pdf");
        let target2 = temp_dir.path().join("target2.pdf");
        std::fs::write(&target1, b"Content 1").unwrap();
        std::fs::write(&target2, b"Content 2").unwrap();

        // Create first symlink
        let symlink_path1 = manager.create_symlink(&target1, "links").unwrap();
        assert!(symlink_path1.exists());

        // Rename target2 to have same name as target1
        let target2_renamed = temp_dir.path().join("target1.pdf");
        // Actually, let's create the symlink to target2 with same name
        std::fs::rename(&target2, &target2_renamed).unwrap();

        // Create second symlink with same name - should overwrite
        let symlink_path2 = manager.create_symlink(&target2_renamed, "links").unwrap();

        // Should still exist and point to new content
        let content = std::fs::read(&symlink_path2).unwrap();
        assert_eq!(content, b"Content 2");
    }

    #[test]
    fn test_symlink_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create target file
        let target = temp_dir.path().join("target.pdf");
        std::fs::write(&target, b"Content").unwrap();

        // Symlink to non-existent directory
        let symlink_path = manager.create_symlink(&target, "new/nested/dir").unwrap();

        assert!(symlink_path.exists());
        assert!(temp_dir.path().join("new/nested/dir").exists());
    }

    #[test]
    fn test_symlink_preserves_filename() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create target file with specific name
        let target = temp_dir.path().join("my-document.pdf");
        std::fs::write(&target, b"Content").unwrap();

        let symlink_path = manager.create_symlink(&target, "links").unwrap();

        // Symlink should have same filename as target
        assert_eq!(
            symlink_path.file_name().unwrap().to_str().unwrap(),
            "my-document.pdf"
        );
    }

    #[test]
    fn test_relative_path_same_directory() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create files in same directory
        let dir = temp_dir.path().join("same");
        std::fs::create_dir_all(&dir).unwrap();

        let from = dir.join("link");
        let to = dir.join("target.txt");
        std::fs::write(&to, "test").unwrap();

        let relative = manager.calculate_relative_path(&from, &to).unwrap();

        // Should be just the filename when in same directory
        assert!(!relative.to_string_lossy().contains(".."));
    }

    #[test]
    fn test_multiple_symlinks_same_target() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SymlinkManager::new(temp_dir.path());

        // Create target file
        let target = temp_dir.path().join("target.pdf");
        std::fs::write(&target, b"Shared content").unwrap();

        // Create multiple symlinks pointing to same target
        let symlink1 = manager.create_symlink(&target, "links/category1").unwrap();
        let symlink2 = manager.create_symlink(&target, "links/category2").unwrap();

        // Both should exist and read the same content
        assert!(symlink1.exists());
        assert!(symlink2.exists());
        assert_eq!(std::fs::read(&symlink1).unwrap(), b"Shared content");
        assert_eq!(std::fs::read(&symlink2).unwrap(), b"Shared content");
    }
}

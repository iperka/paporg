use std::path::{Path, PathBuf};

use chrono::{Datelike, Utc};

use crate::error::StorageError;

/// Move a file from `src` to `dst`. Uses `rename` first (fast, atomic on same
/// filesystem). Falls back to copy + delete when rename fails — this handles
/// cross-device moves and certain macOS permission scenarios.
fn move_file(src: &Path, dst: &Path) -> Result<(), StorageError> {
    // Fast path: atomic rename
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    // Slow path: copy then remove original
    std::fs::copy(src, dst).map_err(|e| StorageError::MoveFile {
        from: src.to_path_buf(),
        to: dst.to_path_buf(),
        source: e,
    })?;
    std::fs::remove_file(src).map_err(|e| StorageError::MoveFile {
        from: src.to_path_buf(),
        to: dst.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

pub struct FileStorage {
    output_directory: PathBuf,
}

impl FileStorage {
    pub fn new<P: AsRef<Path>>(output_directory: P) -> Self {
        Self {
            output_directory: output_directory.as_ref().to_path_buf(),
        }
    }

    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    pub fn store(
        &self,
        content: &[u8],
        relative_directory: &str,
        filename: &str,
        extension: &str,
    ) -> Result<PathBuf, StorageError> {
        let dir_path = self.output_directory.join(relative_directory);
        self.ensure_directory(&dir_path)?;

        let full_filename = format!("{}.{}", filename, extension);

        // Try atomic file creation with O_EXCL to avoid TOCTOU race conditions
        let file_path = self.store_with_atomic_creation(&dir_path, &full_filename, content)?;

        Ok(file_path)
    }

    /// Stores content using atomic file creation to avoid race conditions.
    /// Falls back to resolve_conflict + write if atomic creation fails.
    fn store_with_atomic_creation(
        &self,
        dir_path: &Path,
        filename: &str,
        content: &[u8],
    ) -> Result<PathBuf, StorageError> {
        use std::io::Write;

        let (base, ext) = if let Some(dot_pos) = filename.rfind('.') {
            (&filename[..dot_pos], Some(&filename[dot_pos..]))
        } else {
            (filename, None)
        };

        // Try original filename first, then numbered variants
        for counter in 1..=1000 {
            let try_filename = if counter == 1 {
                filename.to_string()
            } else {
                match ext {
                    Some(ext) => format!("{}_{}{}", base, counter, ext),
                    None => format!("{}_{}", base, counter),
                }
            };

            let try_path = dir_path.join(&try_filename);

            // Use OpenOptions with create_new for atomic creation (O_CREAT | O_EXCL)
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true) // Fails if file exists - atomic check-and-create
                .open(&try_path)
            {
                Ok(mut file) => {
                    // File was created exclusively, now write content
                    file.write_all(content)
                        .map_err(|e| StorageError::WriteFile {
                            path: try_path.clone(),
                            source: e,
                        })?;
                    return Ok(try_path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // File exists, try next number
                    continue;
                }
                Err(e) => {
                    return Err(StorageError::WriteFile {
                        path: try_path,
                        source: e,
                    });
                }
            }
        }

        // Exhausted all attempts
        Err(StorageError::FileExists(dir_path.join(filename)))
    }

    pub fn archive_source<P: AsRef<Path>>(
        &self,
        source_path: P,
        input_directory: &Path,
    ) -> Result<PathBuf, StorageError> {
        let source_path = source_path.as_ref();

        // Create archive directory inside input directory
        let archive_dir = input_directory.join("archive");
        self.ensure_directory(&archive_dir)?;

        // Generate archive filename with date prefix
        let now = Utc::now();
        let date_prefix = format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day());

        let original_name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document");

        let archive_filename = format!("{}_{}", date_prefix, original_name);
        let archive_path = self.resolve_conflict(&archive_dir, &archive_filename)?;

        move_file(source_path, &archive_path)?;

        Ok(archive_path)
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), StorageError> {
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| StorageError::CreateDirectory {
                path: path.to_path_buf(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Resolves filename conflicts by finding an available name.
    /// Note: This function returns a candidate path. The actual file creation
    /// in `store()` should handle the case where another process creates the file
    /// between this check and the write operation.
    fn resolve_conflict(&self, directory: &Path, filename: &str) -> Result<PathBuf, StorageError> {
        let path = directory.join(filename);

        // First, try the original filename - use symlink_metadata to avoid following symlinks
        // and to properly detect if path exists (including broken symlinks)
        if std::fs::symlink_metadata(&path).is_err() {
            return Ok(path);
        }

        // Extract base name and extension
        let (base, ext) = if let Some(dot_pos) = filename.rfind('.') {
            (&filename[..dot_pos], Some(&filename[dot_pos..]))
        } else {
            (filename, None)
        };

        // Try appending numbers until we find an available name
        for counter in 2..=1000 {
            let new_filename = match ext {
                Some(ext) => format!("{}_{}{}", base, counter, ext),
                None => format!("{}_{}", base, counter),
            };

            let new_path = directory.join(&new_filename);
            if std::fs::symlink_metadata(&new_path).is_err() {
                return Ok(new_path);
            }
        }

        Err(StorageError::FileExists(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_file() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        let content = b"Hello, World!";
        let path = storage
            .store(content, "2026/invoices", "test", "pdf")
            .unwrap();

        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), content);
    }

    #[test]
    fn test_store_file_conflict_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        // Store first file
        let path1 = storage.store(b"First", "test", "document", "pdf").unwrap();
        assert!(path1.ends_with("document.pdf"));

        // Store second file with same name - should get _2 suffix
        let path2 = storage.store(b"Second", "test", "document", "pdf").unwrap();
        assert!(path2.ends_with("document_2.pdf"));

        // Store third file with same name - should get _3 suffix
        let path3 = storage.store(b"Third", "test", "document", "pdf").unwrap();
        assert!(path3.ends_with("document_3.pdf"));
    }

    #[test]
    fn test_archive_source() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();

        let storage = FileStorage::new(&output_dir);

        // Create a source file
        let source_file = input_dir.join("test.pdf");
        std::fs::write(&source_file, b"Test content").unwrap();

        // Archive it
        let archive_path = storage.archive_source(&source_file, &input_dir).unwrap();

        // Source should no longer exist
        assert!(!source_file.exists());

        // Archive should exist
        assert!(archive_path.exists());
        assert!(archive_path.starts_with(input_dir.join("archive")));

        // Filename should have date prefix
        let filename = archive_path.file_name().unwrap().to_str().unwrap();
        assert!(filename.contains("test.pdf"));
        assert!(filename.starts_with(&Utc::now().format("%Y-%m-%d").to_string()));
    }

    #[test]
    fn test_create_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        let content = b"Test";
        let path = storage
            .store(content, "deep/nested/directory/structure", "file", "txt")
            .unwrap();

        assert!(path.exists());
        assert!(path.starts_with(temp_dir.path().join("deep/nested/directory/structure")));
    }

    #[test]
    fn test_conflict_resolution_numbering_sequence() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        // Create files with same name to trigger conflict resolution
        for i in 1..=5 {
            let content = format!("Content {}", i);
            let path = storage
                .store(content.as_bytes(), "test", "document", "pdf")
                .unwrap();
            assert!(path.exists());
        }

        // Verify files exist with expected naming
        assert!(temp_dir.path().join("test/document.pdf").exists());
        assert!(temp_dir.path().join("test/document_2.pdf").exists());
        assert!(temp_dir.path().join("test/document_3.pdf").exists());
        assert!(temp_dir.path().join("test/document_4.pdf").exists());
        assert!(temp_dir.path().join("test/document_5.pdf").exists());
    }

    #[test]
    fn test_conflict_resolution_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        // Use resolve_conflict directly on a file without extension
        std::fs::create_dir_all(temp_dir.path().join("test")).unwrap();
        std::fs::write(temp_dir.path().join("test/noext"), b"original").unwrap();

        let resolved = storage
            .resolve_conflict(&temp_dir.path().join("test"), "noext")
            .unwrap();

        assert!(resolved.to_string_lossy().contains("noext_2"));
    }

    #[test]
    fn test_archive_creates_archive_directory() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();

        let storage = FileStorage::new(&output_dir);

        // Create a source file
        let source_file = input_dir.join("test.pdf");
        std::fs::write(&source_file, b"Test content").unwrap();

        // Archive should create the archive directory if it doesn't exist
        let archive_path = storage.archive_source(&source_file, &input_dir).unwrap();

        assert!(input_dir.join("archive").exists());
        assert!(archive_path.exists());
    }

    #[test]
    fn test_archive_missing_source_error() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();

        let storage = FileStorage::new(&output_dir);

        // Try to archive a non-existent file
        let result = storage.archive_source(&input_dir.join("nonexistent.pdf"), &input_dir);

        assert!(result.is_err());
        match result {
            Err(StorageError::MoveFile { from, .. }) => {
                assert!(from.to_string_lossy().contains("nonexistent.pdf"));
            }
            _ => panic!("Expected MoveFile error"),
        }
    }

    #[test]
    fn test_output_directory_accessor() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        assert_eq!(storage.output_directory(), temp_dir.path());
    }

    #[test]
    fn test_store_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path());

        let path = storage.store(&[], "empty", "file", "bin").unwrap();

        assert!(path.exists());
        let content = std::fs::read(&path).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn test_archive_conflict_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::create_dir_all(&input_dir.join("archive")).unwrap();

        let storage = FileStorage::new(&output_dir);

        // Create first file and archive it
        let source1 = input_dir.join("test.pdf");
        std::fs::write(&source1, b"Content 1").unwrap();
        let archive1 = storage.archive_source(&source1, &input_dir).unwrap();

        // Create second file with same name and archive it
        let source2 = input_dir.join("test.pdf");
        std::fs::write(&source2, b"Content 2").unwrap();
        let archive2 = storage.archive_source(&source2, &input_dir).unwrap();

        // Both should exist with different names
        assert!(archive1.exists());
        assert!(archive2.exists());
        assert_ne!(archive1, archive2);
    }
}

//! FileSystem trait definition

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Metadata about a file
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub file_type: FileType,
}

/// Type of file system entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

impl FileMetadata {
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    pub fn len(&self) -> u64 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// A directory entry returned by read_dir
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub file_type: FileType,
}

impl DirEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> &str {
        &self.name
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

/// Abstraction over file system operations for testability
pub trait FileSystem: Send + Sync {
    /// Check if a path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if path is a directory
    fn is_dir(&self, path: &Path) -> bool;

    /// Check if path is a file
    fn is_file(&self, path: &Path) -> bool;

    /// Get file/directory metadata
    fn metadata(&self, path: &Path) -> Result<FileMetadata>;

    /// Read file contents as string
    fn read_to_string(&self, path: &Path) -> Result<String>;

    /// Read first N bytes of file (for binary detection)
    fn read_bytes(&self, path: &Path, max_bytes: usize) -> Result<Vec<u8>>;

    /// List directory contents
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;

    /// Canonicalize a path
    fn canonicalize(&self, path: &Path) -> Result<PathBuf>;

    /// Join paths
    fn join(&self, base: &Path, path: &str) -> PathBuf {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_metadata_is_file() {
        let meta = FileMetadata {
            size: 100,
            file_type: FileType::File,
        };
        assert!(meta.is_file());
        assert!(!meta.is_dir());
    }

    #[test]
    fn test_file_metadata_is_dir() {
        let meta = FileMetadata {
            size: 0,
            file_type: FileType::Directory,
        };
        assert!(meta.is_dir());
        assert!(!meta.is_file());
    }

    #[test]
    fn test_dir_entry() {
        let entry = DirEntry {
            path: PathBuf::from("/test/file.txt"),
            name: "file.txt".to_string(),
            file_type: FileType::File,
        };
        assert_eq!(entry.path(), Path::new("/test/file.txt"));
        assert_eq!(entry.file_name(), "file.txt");
        assert_eq!(entry.file_type(), FileType::File);
    }
}

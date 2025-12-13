use super::{DirEntry, FileMetadata, FileSystem, FileType};
use anyhow::{Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct RealFileSystem;

impl RealFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RealFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for RealFileSystem {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata> {
        let meta = fs::metadata(path).context(format!("Failed to get metadata for {:?}", path))?;

        let file_type = if meta.is_file() {
            FileType::File
        } else if meta.is_dir() {
            FileType::Directory
        } else {
            FileType::Symlink
        };

        Ok(FileMetadata {
            size: meta.len(),
            file_type,
        })
    }

    fn read_to_string(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).context(format!("Failed to read file {:?}", path))
    }

    fn read_bytes(&self, path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
        let mut file = fs::File::open(path).context(format!("Failed to open file {:?}", path))?;
        let mut buffer = vec![0u8; max_bytes];
        let bytes_read = file
            .read(&mut buffer)
            .context(format!("Failed to read bytes from {:?}", path))?;
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>> {
        let entries = fs::read_dir(path).context(format!("Failed to read directory {:?}", path))?;

        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = if path.is_file() {
                FileType::File
            } else if path.is_dir() {
                FileType::Directory
            } else {
                FileType::Symlink
            };

            result.push(DirEntry {
                path,
                name,
                file_type,
            });
        }

        Ok(result)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        path.canonicalize()
            .context(format!("Failed to canonicalize path {:?}", path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::create_dir(base.join("subdir")).unwrap();
        fs::File::create(base.join("test.txt"))
            .unwrap()
            .write_all(b"hello world")
            .unwrap();
        fs::File::create(base.join("subdir/nested.txt"))
            .unwrap()
            .write_all(b"nested content")
            .unwrap();

        dir
    }

    #[test]
    fn test_exists() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        assert!(fs.exists(temp.path()));
        assert!(fs.exists(&temp.path().join("test.txt")));
        assert!(!fs.exists(&temp.path().join("nonexistent")));
    }

    #[test]
    fn test_is_dir() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        assert!(fs.is_dir(temp.path()));
        assert!(fs.is_dir(&temp.path().join("subdir")));
        assert!(!fs.is_dir(&temp.path().join("test.txt")));
    }

    #[test]
    fn test_is_file() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        assert!(fs.is_file(&temp.path().join("test.txt")));
        assert!(!fs.is_file(temp.path()));
    }

    #[test]
    fn test_metadata() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        let meta = fs.metadata(&temp.path().join("test.txt")).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.len(), 11); // "hello world"

        let meta = fs.metadata(&temp.path().join("subdir")).unwrap();
        assert!(meta.is_dir());
    }

    #[test]
    fn test_read_to_string() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        let content = fs.read_to_string(&temp.path().join("test.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_read_bytes() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        let bytes = fs.read_bytes(&temp.path().join("test.txt"), 5).unwrap();
        assert_eq!(bytes, b"hello");

        let bytes = fs.read_bytes(&temp.path().join("test.txt"), 100).unwrap();
        assert_eq!(bytes, b"hello world");
    }

    #[test]
    fn test_read_dir() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        let entries = fs.read_dir(temp.path()).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.file_name()).collect();

        assert!(names.contains(&"test.txt"));
        assert!(names.contains(&"subdir"));
    }

    #[test]
    fn test_canonicalize() {
        let temp = create_test_dir();
        let fs = RealFileSystem::new();

        let canonical = fs.canonicalize(temp.path()).unwrap();
        assert!(canonical.is_absolute());
    }
}

use super::{DirEntry, FileMetadata, FileSystem, FileType};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct MockEntry {
    pub content: Option<String>,
    pub file_type: FileType,
}

pub struct MockFileSystem {
    files: RwLock<HashMap<PathBuf, MockEntry>>,
    root: PathBuf,
}

impl MockFileSystem {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            root: PathBuf::from("/mock"),
        }
    }

    pub fn with_root(root: PathBuf) -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            root,
        }
    }

    pub fn add_file(&self, path: impl AsRef<Path>, content: &str) {
        let path = self.normalize_path(path.as_ref());
        let mut files = self.files.write().unwrap();

        if let Some(parent) = path.parent() {
            self.ensure_parents(&mut files, parent);
        }

        files.insert(
            path,
            MockEntry {
                content: Some(content.to_string()),
                file_type: FileType::File,
            },
        );
    }

    pub fn add_dir(&self, path: impl AsRef<Path>) {
        let path = self.normalize_path(path.as_ref());
        let mut files = self.files.write().unwrap();

        self.ensure_parents(&mut files, &path);

        files.insert(
            path,
            MockEntry {
                content: None,
                file_type: FileType::Directory,
            },
        );
    }

    fn normalize_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    fn ensure_parents(&self, files: &mut HashMap<PathBuf, MockEntry>, path: &Path) {
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component);
            if !files.contains_key(&current) {
                files.insert(
                    current.clone(),
                    MockEntry {
                        content: None,
                        file_type: FileType::Directory,
                    },
                );
            }
        }
    }
}

impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for MockFileSystem {
    fn exists(&self, path: &Path) -> bool {
        let path = self.normalize_path(path);
        self.files.read().unwrap().contains_key(&path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        let path = self.normalize_path(path);
        self.files
            .read()
            .unwrap()
            .get(&path)
            .map(|e| e.file_type == FileType::Directory)
            .unwrap_or(false)
    }

    fn is_file(&self, path: &Path) -> bool {
        let path = self.normalize_path(path);
        self.files
            .read()
            .unwrap()
            .get(&path)
            .map(|e| e.file_type == FileType::File)
            .unwrap_or(false)
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata> {
        let path = self.normalize_path(path);
        let files = self.files.read().unwrap();
        let entry = files
            .get(&path)
            .ok_or_else(|| anyhow!("Path not found: {:?}", path))?;

        Ok(FileMetadata {
            size: entry.content.as_ref().map(|c| c.len() as u64).unwrap_or(0),
            file_type: entry.file_type,
        })
    }

    fn read_to_string(&self, path: &Path) -> Result<String> {
        let path = self.normalize_path(path);
        let files = self.files.read().unwrap();
        let entry = files
            .get(&path)
            .ok_or_else(|| anyhow!("File not found: {:?}", path))?;

        entry
            .content
            .clone()
            .ok_or_else(|| anyhow!("Not a file: {:?}", path))
    }

    fn read_bytes(&self, path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
        let content = self.read_to_string(path)?;
        let bytes = content.as_bytes();
        Ok(bytes[..bytes.len().min(max_bytes)].to_vec())
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>> {
        let path = self.normalize_path(path);
        let files = self.files.read().unwrap();

        if !files.contains_key(&path) {
            return Err(anyhow!("Directory not found: {:?}", path));
        }

        let mut entries = Vec::new();
        for (file_path, entry) in files.iter() {
            if let Some(parent) = file_path.parent() {
                if parent == path && file_path != &path {
                    let name = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    entries.push(DirEntry {
                        path: file_path.clone(),
                        name,
                        file_type: entry.file_type,
                    });
                }
            }
        }

        Ok(entries)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        let normalized = self.normalize_path(path);
        if self.files.read().unwrap().contains_key(&normalized) {
            Ok(normalized)
        } else {
            Err(anyhow!("Path not found: {:?}", path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_file() {
        let fs = MockFileSystem::new();
        fs.add_file("test.txt", "hello");

        assert!(fs.exists(Path::new("/mock/test.txt")));
        assert!(fs.is_file(Path::new("/mock/test.txt")));
    }

    #[test]
    fn test_add_dir() {
        let fs = MockFileSystem::new();
        fs.add_dir("subdir");

        assert!(fs.exists(Path::new("/mock/subdir")));
        assert!(fs.is_dir(Path::new("/mock/subdir")));
    }

    #[test]
    fn test_read_to_string() {
        let fs = MockFileSystem::new();
        fs.add_file("test.txt", "hello world");

        let content = fs.read_to_string(Path::new("/mock/test.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_metadata() {
        let fs = MockFileSystem::new();
        fs.add_file("test.txt", "hello");

        let meta = fs.metadata(Path::new("/mock/test.txt")).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.len(), 5);
    }

    #[test]
    fn test_read_dir() {
        let fs = MockFileSystem::new();
        fs.add_dir("subdir");
        fs.add_file("test.txt", "content");
        fs.add_file("subdir/nested.txt", "nested");

        let entries = fs.read_dir(Path::new("/mock")).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.file_name()).collect();

        assert!(names.contains(&"test.txt"));
        assert!(names.contains(&"subdir"));
    }

    #[test]
    fn test_read_bytes() {
        let fs = MockFileSystem::new();
        fs.add_file("test.txt", "hello world");

        let bytes = fs.read_bytes(Path::new("/mock/test.txt"), 5).unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn test_with_root() {
        let fs = MockFileSystem::with_root(PathBuf::from("/repo"));
        fs.add_file("src/main.rs", "fn main() {}");

        assert!(fs.exists(Path::new("/repo/src/main.rs")));
        let content = fs.read_to_string(Path::new("/repo/src/main.rs")).unwrap();
        assert_eq!(content, "fn main() {}");
    }

    #[test]
    fn test_parent_directories_created() {
        let fs = MockFileSystem::new();
        fs.add_file("a/b/c/file.txt", "content");

        assert!(fs.is_dir(Path::new("/mock/a")));
        assert!(fs.is_dir(Path::new("/mock/a/b")));
        assert!(fs.is_dir(Path::new("/mock/a/b/c")));
        assert!(fs.is_file(Path::new("/mock/a/b/c/file.txt")));
    }
}

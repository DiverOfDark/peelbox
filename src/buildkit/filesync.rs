use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncReadExt;
use tracing::{debug, trace};

/// Maximum file chunk size for streaming (1MB)
const CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct FileStat {
    pub path: PathBuf,
    pub size: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub mod_time: i64,
    pub linkname: Option<String>,
    pub is_dir: bool,
}

pub struct FileSync {
    root_path: PathBuf,
}

impl FileSync {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    /// Scan directory and collect file stats
    pub async fn scan_files(&self) -> Result<Vec<FileStat>> {
        debug!("Scanning files in: {:?}", self.root_path);
        let mut stats = Vec::new();

        let walker = WalkBuilder::new(&self.root_path)
            .hidden(false)
            .git_ignore(true)
            .git_global(false)  // Ignore global gitignore to match LLB exclude patterns
            .git_exclude(false) // Ignore .git/info/exclude to match LLB exclude patterns
            .build();

        for entry in walker {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            // Skip root directory
            if path == self.root_path {
                continue;
            }

            let metadata = entry.metadata().context("Failed to read metadata")?;
            let relative_path = path
                .strip_prefix(&self.root_path)
                .context("Failed to strip prefix")?
                .to_path_buf();

            let stat = FileStat {
                path: relative_path.clone(),
                size: metadata.len(),
                mode: Self::get_file_mode(&metadata),
                uid: Self::get_uid(&metadata),
                gid: Self::get_gid(&metadata),
                mod_time: Self::get_mod_time(&metadata),
                linkname: Self::get_linkname(path).await,
                is_dir: metadata.is_dir(),
            };

            trace!("Scanned: {:?} (size: {}, mode: {:o})", relative_path, stat.size, stat.mode);
            stats.push(stat);
        }

        debug!("Scanned {} files/directories", stats.len());

        // Sort files by path for BuildKit's fsutil.Validator
        // Use simple lexicographic ordering - BuildKit's validator expects
        // entries sorted by path within each directory
        stats.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(stats)
    }

    /// Read file content in chunks
    pub async fn read_file_chunks(&self, relative_path: &Path) -> Result<Vec<Vec<u8>>> {
        let full_path = self.root_path.join(relative_path);
        debug!("Reading file: {:?}", full_path);

        let mut file = fs::File::open(&full_path)
            .await
            .context("Failed to open file")?;

        let metadata = file.metadata().await.context("Failed to read metadata")?;
        let file_size = metadata.len() as usize;

        let mut chunks = Vec::new();
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut total_read = 0;

        loop {
            let n = file.read(&mut buffer).await.context("Failed to read file")?;
            if n == 0 {
                break;
            }

            chunks.push(buffer[..n].to_vec());
            total_read += n;

            trace!("Read chunk: {} bytes (total: {}/{})", n, total_read, file_size);
        }

        debug!("Read {} chunks ({} bytes total)", chunks.len(), total_read);
        Ok(chunks)
    }

    /// Get Unix file mode from metadata
    #[cfg(unix)]
    fn get_file_mode(metadata: &std::fs::Metadata) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        Self::unix_mode_to_go_filemode(metadata.permissions().mode(), metadata.is_dir())
    }

    #[cfg(not(unix))]
    fn get_file_mode(metadata: &std::fs::Metadata) -> u32 {
        // Default permissions for non-Unix systems
        if metadata.is_dir() {
            0x80000000 | 0o755 // Go's os.ModeDir | 0755
        } else {
            0o644
        }
    }

    /// Convert Unix st_mode to Go's os.FileMode format
    ///
    /// Go's os.FileMode uses different bit positions than Unix:
    /// - Unix S_IFDIR (0x4000) -> Go os.ModeDir (0x80000000 = bit 31)
    /// - Unix S_IFREG (0x8000) -> No special bit in Go (just permissions)
    /// - Unix S_IFLNK (0xa000) -> Go os.ModeSymlink (0x08000000 = bit 27)
    #[cfg(unix)]
    fn unix_mode_to_go_filemode(unix_mode: u32, is_dir: bool) -> u32 {
        const S_IFMT: u32 = 0xf000;  // Unix file type mask
        const S_IFDIR: u32 = 0x4000; // Unix directory
        const S_IFLNK: u32 = 0xa000; // Unix symlink

        const GO_MODE_DIR: u32 = 0x80000000;     // Go os.ModeDir (bit 31)
        const GO_MODE_SYMLINK: u32 = 0x08000000; // Go os.ModeSymlink (bit 27)
        const GO_MODE_PERM: u32 = 0x1ff;         // Permission bits (0777)

        // Extract permission bits (lower 9 bits: rwxrwxrwx)
        let perm = unix_mode & GO_MODE_PERM;

        // Extract file type
        let file_type = unix_mode & S_IFMT;

        // Convert to Go's FileMode
        match file_type {
            S_IFDIR => GO_MODE_DIR | perm,           // Directory: 0x80000000 | perms
            S_IFLNK => GO_MODE_SYMLINK | perm,       // Symlink: 0x08000000 | perms
            _ if is_dir => GO_MODE_DIR | perm,       // Fallback: check metadata
            _ => perm,                                // Regular file: just permissions
        }
    }

    /// Get user ID from metadata
    #[cfg(unix)]
    fn get_uid(metadata: &std::fs::Metadata) -> u32 {
        use std::os::unix::fs::MetadataExt;
        metadata.uid()
    }

    #[cfg(not(unix))]
    fn get_uid(_metadata: &std::fs::Metadata) -> u32 {
        0
    }

    /// Get group ID from metadata
    #[cfg(unix)]
    fn get_gid(metadata: &std::fs::Metadata) -> u32 {
        use std::os::unix::fs::MetadataExt;
        metadata.gid()
    }

    #[cfg(not(unix))]
    fn get_gid(_metadata: &std::fs::Metadata) -> u32 {
        0
    }

    /// Get modification time as Unix timestamp
    #[cfg(unix)]
    fn get_mod_time(metadata: &std::fs::Metadata) -> i64 {
        use std::os::unix::fs::MetadataExt;
        metadata.mtime()
    }

    #[cfg(not(unix))]
    fn get_mod_time(metadata: &std::fs::Metadata) -> i64 {
        use std::time::UNIX_EPOCH;
        metadata
            .modified()
            .unwrap_or(UNIX_EPOCH)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// Get symlink target if file is a symlink
    async fn get_linkname(path: &Path) -> Option<String> {
        if let Ok(metadata) = tokio::fs::symlink_metadata(path).await {
            if metadata.is_symlink() {
                if let Ok(target) = tokio::fs::read_link(path).await {
                    return Some(target.to_string_lossy().to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_scan_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sync = FileSync::new(temp_dir.path());
        let stats = sync.scan_files().await.unwrap();
        assert_eq!(stats.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"hello world").unwrap();

        let sync = FileSync::new(temp_dir.path());
        let stats = sync.scan_files().await.unwrap();

        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].path, PathBuf::from("test.txt"));
        assert_eq!(stats[0].size, 11);
        assert!(!stats[0].is_dir);
    }

    #[tokio::test]
    async fn test_scan_with_subdirectories() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("nested.txt"), b"nested").unwrap();

        let sync = FileSync::new(temp_dir.path());
        let stats = sync.scan_files().await.unwrap();

        assert_eq!(stats.len(), 2); // subdir + nested.txt
        assert!(stats.iter().any(|s| s.path == PathBuf::from("subdir") && s.is_dir));
        assert!(stats.iter().any(|s| s.path == PathBuf::from("subdir/nested.txt") && !s.is_dir));
    }

    #[tokio::test]
    async fn test_read_file_chunks() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = b"hello world from filesync";
        fs::write(&file_path, content).unwrap();

        let sync = FileSync::new(temp_dir.path());
        let chunks = sync.read_file_chunks(Path::new("test.txt")).await.unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0], content);
    }

    #[tokio::test]
    async fn test_read_large_file_chunks() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");

        // Create a file larger than CHUNK_SIZE (1MB)
        let content = vec![b'x'; CHUNK_SIZE + 100];
        fs::write(&file_path, &content).unwrap();

        let sync = FileSync::new(temp_dir.path());
        let chunks = sync.read_file_chunks(Path::new("large.txt")).await.unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), CHUNK_SIZE);
        assert_eq!(chunks[1].len(), 100);
    }
}

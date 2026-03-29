//! PHP version detection from `.php-version` file.
//!
//! Reads the `.php-version` file and extracts the major.minor version (e.g., "8.2").

use std::path::Path;

/// Read PHP version from `.php-version` file.
/// Returns the major.minor version string (e.g., "8.2").
pub fn read_php_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    for dir in &[project_dir, repo_root] {
        let path = dir.join(".php-version");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            // Extract major.minor (e.g., "8.2.15" -> "8.2", "8.2" -> "8.2")
            let parts: Vec<&str> = trimmed.split('.').collect();
            if parts.len() >= 2
                && parts[0].chars().all(|c| c.is_ascii_digit())
                && parts[1].chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("{}.{}", parts[0], parts[1]));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_php_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".php-version"), "8.2.15\n").unwrap();
        assert_eq!(
            read_php_version(dir.path(), dir.path()),
            Some("8.2".to_string())
        );
    }

    #[test]
    fn test_read_php_version_major_minor_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".php-version"), "8.3\n").unwrap();
        assert_eq!(
            read_php_version(dir.path(), dir.path()),
            Some("8.3".to_string())
        );
    }

    #[test]
    fn test_read_php_version_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_php_version(dir.path(), dir.path()), None);
    }
}

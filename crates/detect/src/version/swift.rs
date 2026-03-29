//! Swift version detection from `.swift-version` file.

use std::path::Path;

/// Read Swift version from `.swift-version` file.
/// Returns the full version string (e.g., "5.9.2").
pub fn read_swift_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    for dir in &[project_dir, repo_root] {
        let path = dir.join(".swift-version");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            if !trimmed.is_empty()
                && trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_swift_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".swift-version"), "5.9.2\n").unwrap();
        assert_eq!(
            read_swift_version(dir.path(), dir.path()),
            Some("5.9.2".to_string())
        );
    }

    #[test]
    fn test_read_swift_version_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_swift_version(dir.path(), dir.path()), None);
    }

    #[test]
    fn test_read_swift_version_non_numeric() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".swift-version"), "# comment\n").unwrap();
        assert_eq!(read_swift_version(dir.path(), dir.path()), None);
    }
}

//! Python version detection from `.python-version`, `runtime.txt`, and `Pipfile`.
//!
//! Priority order:
//! 1. `.python-version` file (highest) -- plain text `X.Y.Z`
//! 2. `runtime.txt` (Heroku convention) -- `python-X.Y.Z`
//! 3. `Pipfile` -- `[requires]` section with `python_version = "X.Y"`
//!
//! Note: Version constraints from `pyproject.toml` (e.g., `^3.9`, `>=3.10`) are NOT
//! valid for package versioning -- only exact versions from the above files are used.

use std::path::Path;

/// Read Python version from `.python-version` file, `runtime.txt`, or `Pipfile`.
/// Returns the major.minor version string (e.g., "3.11").
pub fn read_python_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    // Check .python-version first (highest priority)
    for dir in &[project_dir, repo_root] {
        let path = dir.join(".python-version");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            // Extract major.minor (e.g., "3.11.4" -> "3.11")
            let parts: Vec<&str> = trimmed.split('.').collect();
            if parts.len() >= 2
                && parts[0].chars().all(|c| c.is_ascii_digit())
                && parts[1].chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("{}.{}", parts[0], parts[1]));
            }
        }
    }

    // Check runtime.txt (Heroku convention: "python-3.11.4" or "python-2.7")
    for dir in &[project_dir, repo_root] {
        let path = dir.join("runtime.txt");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            if let Some(version_str) = trimmed.strip_prefix("python-") {
                let parts: Vec<&str> = version_str.split('.').collect();
                if parts.len() >= 2
                    && parts[0].chars().all(|c| c.is_ascii_digit())
                    && parts[1].chars().all(|c| c.is_ascii_digit())
                {
                    return Some(format!("{}.{}", parts[0], parts[1]));
                }
            }
        }
    }

    // Check Pipfile for python_version in [requires] section
    for dir in &[project_dir, repo_root] {
        let path = dir.join("Pipfile");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(version) = parse_pipfile_python_version(&content) {
                return Some(version);
            }
        }
    }

    None
}

/// Extract `python_version` from Pipfile `[requires]` section.
/// Matches `python_version = "3.11"` or `python_version = "3"`.
pub fn parse_pipfile_python_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*python_version\s*=\s*["'](\d+\.\d+)["']"#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pipfile_python_version() {
        let content = r#"
[requires]
python_version = "3.11"
"#;
        assert_eq!(
            parse_pipfile_python_version(content),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn test_parse_pipfile_python_version_single_quotes() {
        let content = r#"
[requires]
python_version = '3.10'
"#;
        assert_eq!(
            parse_pipfile_python_version(content),
            Some("3.10".to_string())
        );
    }

    #[test]
    fn test_parse_pipfile_no_version() {
        assert_eq!(
            parse_pipfile_python_version("[packages]\ndjango = \"*\"\n"),
            None
        );
    }

    #[test]
    fn test_read_python_version_from_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "3.11.4\n").unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn test_read_python_version_from_runtime_txt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("runtime.txt"), "python-3.10.12\n").unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.10".to_string())
        );
    }

    #[test]
    fn test_read_python_version_from_pipfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Pipfile"),
            "[requires]\npython_version = \"3.9\"\n",
        )
        .unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.9".to_string())
        );
    }

    #[test]
    fn test_python_version_file_takes_priority() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "3.12.1\n").unwrap();
        std::fs::write(
            dir.path().join("Pipfile"),
            "[requires]\npython_version = \"3.9\"\n",
        )
        .unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.12".to_string())
        );
    }
}

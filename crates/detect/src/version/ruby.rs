//! Ruby version detection from `.ruby-version`, `Gemfile`, and `Gemfile.lock`.
//!
//! Priority order:
//! 1. `.ruby-version` file (highest) -- plain text `X.Y.Z`
//! 2. `Gemfile` -- `ruby 'X.Y.Z'` directive
//! 3. `Gemfile.lock` -- `RUBY VERSION` section

use std::path::Path;

/// Read Ruby version from `.ruby-version` file, Gemfile `ruby` directive, or Gemfile.lock.
/// Returns the major.minor version string (e.g., "3.2").
pub fn read_ruby_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    // 1. .ruby-version file (highest priority)
    for dir in &[project_dir, repo_root] {
        let path = dir.join(".ruby-version");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            let parts: Vec<&str> = trimmed.split('.').collect();
            if parts.len() >= 2
                && parts[0].chars().all(|c| c.is_ascii_digit())
                && parts[1].chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("{}.{}", parts[0], parts[1]));
            }
        }
    }

    // 2. Gemfile `ruby 'X.Y.Z'` directive
    for dir in &[project_dir, repo_root] {
        let path = dir.join("Gemfile");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(version) = parse_gemfile_ruby_version(&content) {
                return Some(version);
            }
        }
    }

    // 3. Gemfile.lock RUBY VERSION section
    for dir in &[project_dir, repo_root] {
        let path = dir.join("Gemfile.lock");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(version) = parse_gemfile_lock_ruby_version(&content) {
                return Some(version);
            }
        }
    }

    None
}

/// Parse `ruby 'X.Y.Z'` or `ruby "X.Y.Z"` from Gemfile content.
pub fn parse_gemfile_ruby_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*ruby\s+['"](\d+\.\d+)(?:\.\d+)?['"]\s*$"#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Parse Ruby version from Gemfile.lock's RUBY VERSION section.
pub fn parse_gemfile_lock_ruby_version(content: &str) -> Option<String> {
    let mut in_ruby_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "RUBY VERSION" {
            in_ruby_section = true;
            continue;
        }
        if in_ruby_section {
            if trimmed.is_empty() || (!trimmed.starts_with(' ') && !trimmed.starts_with("ruby ")) {
                break;
            }
            // Match "ruby X.Y.Zp..." pattern
            if let Some(ver) = trimmed.strip_prefix("ruby ") {
                let parts: Vec<&str> = ver.split('.').collect();
                if parts.len() >= 2
                    && parts[0].chars().all(|c| c.is_ascii_digit())
                    && parts[1].chars().all(|c| c.is_ascii_digit())
                {
                    return Some(format!("{}.{}", parts[0], parts[1]));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemfile_ruby_version() {
        assert_eq!(
            parse_gemfile_ruby_version("ruby '3.2.2'\n"),
            Some("3.2".to_string())
        );
        assert_eq!(
            parse_gemfile_ruby_version("ruby \"3.1.0\"\n"),
            Some("3.1".to_string())
        );
        assert_eq!(parse_gemfile_ruby_version("gem 'rails'\n"), None);
    }

    #[test]
    fn test_parse_gemfile_lock_ruby_version() {
        let content = "\
GEM
  remote: https://rubygems.org/
  specs:
    rails (7.0.4)

RUBY VERSION
   ruby 3.2.2p53

BUNDLED WITH
   2.4.0
";
        assert_eq!(
            parse_gemfile_lock_ruby_version(content),
            Some("3.2".to_string())
        );
    }

    #[test]
    fn test_parse_gemfile_lock_no_ruby_section() {
        let content = "\
GEM
  remote: https://rubygems.org/
  specs:
    rails (7.0.4)

BUNDLED WITH
   2.4.0
";
        assert_eq!(parse_gemfile_lock_ruby_version(content), None);
    }

    #[test]
    fn test_read_ruby_version_from_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".ruby-version"), "3.2.2\n").unwrap();
        assert_eq!(
            read_ruby_version(dir.path(), dir.path()),
            Some("3.2".to_string())
        );
    }

    #[test]
    fn test_read_ruby_version_from_gemfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\nruby '3.1.0'\ngem 'rails'\n",
        )
        .unwrap();
        assert_eq!(
            read_ruby_version(dir.path(), dir.path()),
            Some("3.1".to_string())
        );
    }
}

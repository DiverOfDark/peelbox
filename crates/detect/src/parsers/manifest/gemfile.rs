use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const RUBY: LanguageId = LanguageId::new("ruby");
const BUNDLER: BuildSystemId = BuildSystemId::new("bundler");
const RUBY_RT: RuntimeId = RuntimeId::new("ruby");

inventory::submit! {
    LanguageMeta { slug: "ruby", display_name: "Ruby", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "bundler", display_name: "Bundler", aliases: &["bundler"] }
}
inventory::submit! {
    RuntimeMeta { slug: "ruby", display_name: "Ruby", aliases: &["ruby"] }
}

pub struct GemfileParser;

impl ManifestParser for GemfileParser {
    fn filenames(&self) -> &[&str] {
        &["Gemfile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("gem ") && !content.contains("source ") {
            return None;
        }

        let deps: Vec<Dependency> = content
            .lines()
            .filter_map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with("gem ") {
                    let name = trimmed
                        .trim_start_matches("gem ")
                        .split(',')
                        .next()?
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string();
                    Some(Dependency {
                        name,
                        version: None,
                        scope: DepScope::Runtime,
                        is_internal: false,
                    })
                } else {
                    None
                }
            })
            .collect();

        let has_sqlite = deps.iter().any(|d| d.name == "sqlite3");
        let has_pg = deps.iter().any(|d| d.name == "pg");
        let has_mysql = deps.iter().any(|d| d.name == "mysql2");
        let has_charlock = deps.iter().any(|d| d.name == "charlock_holmes");
        let has_nokogiri = deps.iter().any(|d| d.name == "nokogiri");

        let mut build_packages = vec![
            "ruby".into(),
            "ruby-dev".into(),
            "ruby-bundler".into(),
            "build-base".into(),
            "glibc-dev".into(),
            "linux-headers".into(),
            "libffi-dev".into(),
            "yaml-dev".into(),
            "pkgconf".into(),
            "ca-certificates".into(),
        ];
        if has_sqlite {
            build_packages.push("sqlite-dev".into());
        }
        if has_pg {
            build_packages.push("postgresql-dev".into());
        }
        if has_mysql {
            // mysql2 gem needs both -dev for headers/symlinks and base for libmariadb.so.3
            build_packages.push("mariadb-connector-c-dev".into());
            build_packages.push("mariadb-connector-c".into());
        }
        if has_charlock {
            build_packages.push("cmake".into());
            build_packages.push("icu-dev".into());
            build_packages.push("zlib-dev".into());
        }
        if has_nokogiri {
            build_packages.push("libxml2-dev".into());
            build_packages.push("libxslt-dev".into());
        }

        let mut runtime_packages: Vec<String> = vec![
            "ruby".into(),
            "ruby-bundler".into(),
            "libgcc".into(),
            "libstdc++".into(),
            "busybox".into(),
            "tzdata".into(),
            "ca-certificates".into(),
        ];
        if has_sqlite {
            runtime_packages.push("sqlite-libs".into());
        }
        if has_pg {
            runtime_packages.push("libpq".into());
        }
        if has_mysql {
            runtime_packages.push("mariadb-connector-c".into());
        }
        if has_charlock {
            runtime_packages.push("icu".into());
        }
        if has_nokogiri {
            runtime_packages.push("libxml2".into());
            runtime_packages.push("libxslt".into());
        }

        // Construct Docker Hub build image from Gemfile ruby directive (if present)
        let ruby_version = parse_gemfile_ruby_version(content);
        let build_image = ruby_version
            .as_ref()
            .map(|v| format!("docker.io/library/ruby:{}", v))
            .unwrap_or_else(|| "docker.io/library/ruby:latest".into());

        Some(Manifest {
            path: path.to_path_buf(),
            language: RUBY,
            build_system: BUNDLER,
            runtime: RUBY_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: build_packages,
                commands: vec![
                    // Remove the ruby version constraint from Gemfile — the correct Ruby
                    // version is already installed via Wolfi packages (see read_ruby_version).
                    // Add base64 gem (removed from Ruby 3.4 stdlib, needed by older rack/etc).
                    // If bundle install fails (e.g., native gem incompatible with current
                    // Ruby), extract failing gem name and update just that gem.
                    "sed -i '/^ruby /d' Gemfile && (grep -q \"gem.*'base64'\" Gemfile || echo \"gem 'base64'\" >> Gemfile) && bundle install || { FAILED_GEM=$(bundle install 2>&1 | sed -n 's/.*error occurred while installing \\([^ ]*\\).*/\\1/p'); [ -n \"$FAILED_GEM\" ] && bundle update $FAILED_GEM && bundle install || bundle update && bundle install; }".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("BUNDLE_DEPLOYMENT", "false"),
                    ("BUNDLE_PATH", "vendor/bundle"),
                ]),
                cache_dirs: vec![".bundle".into(), "vendor".into()],
                artifacts: vec![(".".into(), "/app".into())],
                build_image: Some(build_image),
                asset_build: None,
},
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: btree(&[
                    ("BUNDLE_PATH", "vendor/bundle"),
                ]),
                entrypoint: None, // Set by framework detector (Sinatra/Rails)
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(GemfileParser))
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["Ruby"],
        extensions: &["rb"],
        port_patterns: &[
            r#"set\s*:port\s*,\s*(\d{4,5})"#,
            r#"port\s*=\s*(\d{4,5})"#,
        ],
        health_patterns: &[],
        env_var_patterns: &[],
    }
}

// ── Ruby version detection ──────────────────────────────────────────────

/// Read Ruby version from `.ruby-version` file, Gemfile `ruby` directive, or Gemfile.lock.
/// Returns the major.minor version string (e.g., "3.2").
pub(crate) fn read_ruby_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
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
pub(crate) fn parse_gemfile_ruby_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*ruby\s+['"](\d+\.\d+)(?:\.\d+)?['"]\s*$"#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Parse Ruby version from Gemfile.lock's RUBY VERSION section.
pub(crate) fn parse_gemfile_lock_ruby_version(content: &str) -> Option<String> {
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

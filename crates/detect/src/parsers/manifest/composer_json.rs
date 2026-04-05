use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const PHP: LanguageId = LanguageId::new("php");
const COMPOSER: BuildSystemId = BuildSystemId::new("composer");
const PHP_RT: RuntimeId = RuntimeId::new("php");

inventory::submit! {
    LanguageMeta { slug: "php", display_name: "PHP", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "composer", display_name: "Composer", aliases: &["composer"] }
}
inventory::submit! {
    RuntimeMeta { slug: "php", display_name: "PHP", aliases: &["php"] }
}

pub struct ComposerJsonParser;

impl ManifestParser for ComposerJsonParser {
    fn filenames(&self) -> &[&str] {
        &["composer.json"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;

        // Extract PHP version requirement (e.g., ">=8.1" -> "8.1")
        let php_version = json
            .get("require")
            .and_then(|r| r.get("php"))
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let re = regex::Regex::new(r"(\d+\.\d+)").ok()?;
                re.captures(s)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            });

        let php_pkg = php_version
            .as_ref()
            .map(|v| format!("php-{}", v))
            .unwrap_or_else(|| "php".into());

        let deps: Vec<Dependency> = json
            .get("require")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter(|(k, _)| *k != "php")
                    .map(|(k, v)| Dependency {
                        name: k.clone(),
                        version: v.as_str().map(String::from),
                        scope: DepScope::Runtime,
                        is_internal: false,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Detect if Symfony
        let is_symfony = deps.iter().any(|d| d.name.starts_with("symfony/"));
        let has_runtime_plugin = deps.iter().any(|d| d.name == "symfony/runtime");

        // Build PHP extensions list
        let php_extensions: Vec<String> = php_version
            .as_ref()
            .map(|v| {
                vec![
                    format!("php-{}-ctype", v),
                    format!("php-{}-phar", v),
                    format!("php-{}-openssl", v),
                    format!("php-{}-mbstring", v),
                    format!("php-{}-xml", v),
                    format!("php-{}-dom", v),
                    format!("php-{}-curl", v),
                ]
            })
            .unwrap_or_default();

        let mut build_packages = vec![
            php_pkg.clone(),
            "composer".into(),
            "unzip".into(),
            "git".into(),
            "ca-certificates".into(),
        ];
        build_packages.extend(php_extensions.clone());

        let mut build_commands = Vec::new();
        if is_symfony && has_runtime_plugin {
            build_commands.push("composer config allow-plugins.symfony/runtime true".into());
        }
        build_commands
            .push("composer install --no-dev --optimize-autoloader --ignore-platform-reqs".into());
        // Create .env from .env.example if missing (or minimal fallback), then generate APP_KEY
        build_commands.push(
            "[ -f .env ] || cp .env.example .env 2>/dev/null || echo 'APP_KEY=' > .env; php artisan key:generate --force 2>/dev/null || true".into(),
        );

        // Runtime PHP extensions (base + extras)
        let mut runtime_packages = vec![php_pkg.clone()];
        runtime_packages.extend(php_extensions);
        if let Some(v) = &php_version {
            runtime_packages.push(format!("php-{}-fileinfo", v));
            runtime_packages.push(format!("php-{}-iconv", v));
            runtime_packages.push(format!("php-{}-pdo_sqlite", v));
            if is_symfony {
                runtime_packages.push(format!("php-{}-intl", v));
                runtime_packages.push(format!("php-{}-pdo", v));
            }
        }
        runtime_packages.push("busybox".into());
        runtime_packages.push("ca-certificates".into());

        Some(Manifest {
            path: path.to_path_buf(),
            language: PHP,
            build_system: COMPOSER,
            runtime: PHP_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: build_packages,
                commands: build_commands,
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".composer/cache".into()],
                artifacts: vec![(".".into(), "/app".into())],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: BTreeMap::new(),
                entrypoint: Some("php -S 0.0.0.0:8000 index.php".into()),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ComposerJsonParser))
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["PHP"],
        extensions: &["php"],
        port_patterns: &[
            r#"'PORT'\s*,\s*(\d{4,5})"#,
            r#"\$port\s*=\s*(\d{4,5})"#,
        ],
        health_patterns: &[
            r#"\$app->get\(['"]([/\w\-]*health[/\w\-]*)['"]"#,
            r#"case\s+['"]([/\w\-]*health[/\w\-]*)['"]"#,
            r#"Route::get\(['"]([/\w\-]*health[/\w\-]*)['"]"#,
        ],
        env_var_patterns: &[],
    }
}

// ── PHP version detection ───────────────────────────────────────────────

/// Read PHP version from `.php-version` file.
/// Returns the major.minor version string (e.g., "8.2").
pub(crate) fn read_php_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
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

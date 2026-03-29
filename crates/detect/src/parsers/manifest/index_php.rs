use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

// Language / Runtime reuse the same IDs as composer_json.rs (PHP, php).
// The `LanguageMeta` and `RuntimeMeta` are already submitted by composer_json.
const PHP: LanguageId = LanguageId::new("php");
const PHP_NONE: BuildSystemId = BuildSystemId::new("none");
const PHP_RT: RuntimeId = RuntimeId::new("php");

inventory::submit! {
    BuildSystemMeta { slug: "none", display_name: "None", aliases: &[] }
}

/// Detects vanilla PHP projects via `index.php` when no `composer.json` is present.
///
/// This parser produces a minimal manifest: no build commands (just copy),
/// with `php -S 0.0.0.0:8080` as the runtime entrypoint.
pub struct IndexPhpParser;

impl ManifestParser for IndexPhpParser {
    fn filenames(&self) -> &[&str] {
        &["index.php"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        // If composer.json exists in the same directory or any parent directory,
        // defer to ComposerJsonParser. This handles cases like `public/index.php`
        // in Symfony / Laravel projects where composer.json is at the root.
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d.join("composer.json").exists() {
                return None;
            }
            dir = d.parent();
            // Stop at filesystem root
            if dir == Some(Path::new("")) || dir == Some(Path::new("/")) {
                break;
            }
        }

        // If a parent directory also has an index.php, defer to that one.
        // This prevents duplicate "app" service names when a PHP project has
        // index.php in both the root and subdirectories (e.g., stuff/index.php).
        if let Some(current_dir) = path.parent() {
            let mut parent = current_dir.parent();
            while let Some(p) = parent {
                if p.join("index.php").exists() {
                    return None;
                }
                parent = p.parent();
                if parent == Some(Path::new("")) || parent == Some(Path::new("/")) {
                    break;
                }
            }
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: PHP,
            build_system: PHP_NONE,
            runtime: PHP_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: vec![],
            build: BuildSpec {
                packages: vec!["php".into(), "ca-certificates".into()],
                commands: vec!["echo 'No build step required'".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![],
                artifacts: vec![(".".into(), "/app".into())],
                            build_image: None,
},
            runtime_config: RuntimeSpec {
                packages: vec!["php".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some("php -S 0.0.0.0:8080 -t /app".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(IndexPhpParser))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_php_parser_basic() {
        let parser = IndexPhpParser;
        let content = r#"<?php echo "Hello, World!"; ?>"#;
        // Use a temp dir so there's no real composer.json adjacent
        let tmp = tempfile::tempdir().unwrap();
        let index_path = tmp.path().join("index.php");
        std::fs::write(&index_path, content).unwrap();

        let manifest = crate::traits::ManifestParser::parse(&parser, &index_path, content).unwrap();
        assert_eq!(manifest.language, PHP);
        assert_eq!(manifest.build_system, PHP_NONE);
        assert_eq!(manifest.runtime, PHP_RT);
        assert!(manifest.dependencies.is_empty());
        assert_eq!(manifest.runtime_config.ports, vec![8080]);
    }

    #[test]
    fn test_index_php_skipped_when_composer_json_present() {
        let parser = IndexPhpParser;
        let content = r#"<?php require __DIR__ . '/vendor/autoload.php'; ?>"#;
        let tmp = tempfile::tempdir().unwrap();
        let index_path = tmp.path().join("index.php");
        std::fs::write(&index_path, content).unwrap();
        // Create a composer.json alongside
        std::fs::write(tmp.path().join("composer.json"), "{}").unwrap();

        let result = crate::traits::ManifestParser::parse(&parser, &index_path, content);
        assert!(result.is_none(), "Should skip when composer.json exists");
    }

    #[test]
    fn test_index_php_skipped_when_parent_has_index_php() {
        let parser = IndexPhpParser;
        let content = r#"<?php echo "Hello!"; ?>"#;
        let tmp = tempfile::tempdir().unwrap();

        // Create root index.php
        std::fs::write(tmp.path().join("index.php"), content).unwrap();

        // Create subdirectory with its own index.php
        let subdir = tmp.path().join("stuff");
        std::fs::create_dir(&subdir).unwrap();
        let sub_index_path = subdir.join("index.php");
        std::fs::write(&sub_index_path, content).unwrap();

        // Root index.php should be detected
        let root_path = tmp.path().join("index.php");
        let result = crate::traits::ManifestParser::parse(&parser, &root_path, content);
        assert!(result.is_some(), "Root index.php should be detected");

        // Subdirectory index.php should be skipped (parent has index.php)
        let result = crate::traits::ManifestParser::parse(&parser, &sub_index_path, content);
        assert!(
            result.is_none(),
            "Should skip subdirectory index.php when parent has one"
        );
    }
}

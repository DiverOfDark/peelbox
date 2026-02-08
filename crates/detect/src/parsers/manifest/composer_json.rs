use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_stack::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

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

        // Runtime PHP extensions (base + extras)
        let mut runtime_packages = vec![php_pkg.clone()];
        runtime_packages.extend(php_extensions);
        if let Some(v) = &php_version {
            runtime_packages.push(format!("php-{}-fileinfo", v));
            runtime_packages.push(format!("php-{}-iconv", v));
            if is_symfony {
                runtime_packages.push(format!("php-{}-intl", v));
                runtime_packages.push(format!("php-{}-pdo", v));
            }
        }
        runtime_packages.push("ca-certificates".into());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::PHP,
            build_system: BuildSystemId::Composer,
            runtime: RuntimeId::PHP,
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

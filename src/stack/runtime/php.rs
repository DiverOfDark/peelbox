use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct PhpRuntime;

impl PhpRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let env_pattern = Regex::new(r#"\$_ENV\[['"]([A-Z_][A-Z0-9_]*)['"]\]"#).unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "php" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        for cap in env_pattern.captures_iter(&content) {
                            if let Some(var) = cap.get(1) {
                                env_vars.insert(var.as_str().to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut vars: Vec<String> = env_vars.into_iter().collect();
        vars.sort();
        vars
    }

    fn extract_native_deps(&self, files: &[PathBuf]) -> Vec<String> {
        let mut deps = HashSet::new();

        for file in files {
            if file.file_name().is_some_and(|n| n == "composer.json") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if content.contains("ext-")
                        || content.contains("imagick")
                        || content.contains("gd")
                    {
                        deps.insert("build-base".to_string());
                    }
                }
            }
        }

        let mut result: Vec<String> = deps.into_iter().collect();
        result.sort();
        result
    }
}

impl Runtime for PhpRuntime {
    fn name(&self) -> &str {
        "PHP"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let env_vars = self.extract_env_vars(files);
        let native_deps = self.extract_native_deps(files);

        let port = framework.and_then(|f| f.default_ports().first().copied());
        let health = framework.and_then(|f| {
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint: None,
            port,
            env_vars,
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("8.2");
        format!("php:{}-fpm-alpine", version)
    }

    fn required_packages(&self) -> Vec<String> {
        vec![]
    }

    fn start_command(&self, _entrypoint: &Path) -> String {
        "php-fpm".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_php_runtime_name() {
        let runtime = PhpRuntime;
        assert_eq!(runtime.name(), "PHP");
    }

    #[test]
    fn test_php_runtime_base_image_default() {
        let runtime = PhpRuntime;
        assert_eq!(runtime.runtime_base_image(None), "php:8.2-fpm-alpine");
    }

    #[test]
    fn test_php_runtime_base_image_versioned() {
        let runtime = PhpRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("8.3")),
            "php:8.3-fpm-alpine"
        );
    }

    #[test]
    fn test_php_required_packages() {
        let runtime = PhpRuntime;
        let packages: Vec<String> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_php_start_command() {
        let runtime = PhpRuntime;
        let entrypoint = Path::new("index.php");
        assert_eq!(runtime.start_command(entrypoint), "php-fpm");
    }

    #[test]
    fn test_extract_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let php_file = temp_dir.path().join("config.php");
        fs::write(
            &php_file,
            r#"
<?php
$db = $_ENV['DATABASE_URL'];
$key = $_ENV["API_KEY"];
?>
"#,
        )
        .unwrap();

        let runtime = PhpRuntime;
        let files = vec![php_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL"]);
    }

    #[test]
    fn test_extract_native_deps() {
        let temp_dir = TempDir::new().unwrap();
        let composer_file = temp_dir.path().join("composer.json");
        fs::write(
            &composer_file,
            r#"
{
    "require": {
        "ext-gd": "*",
        "monolog/monolog": "^2.0"
    }
}
"#,
        )
        .unwrap();

        let runtime = PhpRuntime;
        let files = vec![composer_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base".to_string()]);
    }
}

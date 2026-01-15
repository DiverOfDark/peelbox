use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct PhpRuntime;

impl PhpRuntime {
    fn find_entrypoint(&self, files: &[PathBuf]) -> Option<String> {
        for file in files {
            let file_str = file.to_string_lossy();

            // Check for public/index.php (Symfony, Laravel, etc.)
            if file_str.ends_with("public/index.php") || file_str == "public/index.php" {
                return Some("/usr/bin/php -S 0.0.0.0:8000 -t /app/public".to_string());
            }
        }
        None
    }

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
        let entrypoint = self.find_entrypoint(files);
        let env_vars = self.extract_env_vars(files);
        let native_deps = self.extract_native_deps(files);

        let port = framework.and_then(|f| f.default_ports().first().copied());
        let health = framework.and_then(|f| {
            f.health_endpoints(files)
                .first()
                .map(|endpoint| HealthCheck {
                    endpoint: endpoint.to_string(),
                })
        });

        Some(RuntimeConfig {
            entrypoint,
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

    fn runtime_packages(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> Vec<String> {
        let requested = self.detect_version(service_path, manifest_content);
        let available = wolfi_index.get_versions("php");

        let version = requested
            .as_deref()
            .and_then(|r| wolfi_index.match_version("php", r, &available))
            .or_else(|| wolfi_index.get_latest_version("php"))
            .unwrap_or_else(|| "php-8.3".to_string());

        let required_extensions = vec![
            "ctype", "phar", "openssl", "mbstring", "xml", "dom", "curl", "fileinfo", "iconv",
        ];

        let framework_extensions = self.detect_framework_extensions(manifest_content);

        let mut packages = vec![version.clone()];
        packages.extend(
            required_extensions
                .iter()
                .map(|ext| format!("{}-{}", version, ext)),
        );
        packages.extend(
            framework_extensions
                .iter()
                .map(|ext| format!("{}-{}", version, ext)),
        );

        packages
    }
}

impl PhpRuntime {
    fn detect_framework_extensions(&self, manifest_content: Option<&str>) -> Vec<String> {
        let mut extensions = HashSet::new();

        if let Some(content) = manifest_content {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                // Check for Laravel
                if json["require"].get("laravel/framework").is_some() {
                    extensions.insert("pdo".to_string());
                    extensions.insert("pdo_mysql".to_string());
                    extensions.insert("redis".to_string());
                    extensions.insert("zip".to_string());
                }

                // Check for Symfony
                if json["require"].get("symfony/framework-bundle").is_some()
                    || json["require"].get("symfony/symfony").is_some()
                {
                    extensions.insert("intl".to_string());
                    extensions.insert("pdo".to_string());
                }

                // Check for WordPress (when using Bedrock or similar composer setups)
                if json["require"].get("wordpress").is_some()
                    || json["require"].get("roots/wordpress").is_some()
                    || json["require"].get("johnpbloch/wordpress").is_some()
                {
                    extensions.insert("mysqli".to_string());
                    extensions.insert("gd".to_string());
                    extensions.insert("zip".to_string());
                }

                // Check for explicitly required extensions in composer.json
                if let Some(require) = json["require"].as_object() {
                    for key in require.keys() {
                        if let Some(ext_name) = key.strip_prefix("ext-") {
                            extensions.insert(ext_name.to_string());
                        }
                    }
                }
            }
        }

        let mut result: Vec<String> = extensions.into_iter().collect();
        result.sort();
        result
    }

    fn detect_version(
        &self,
        _service_path: &Path,
        manifest_content: Option<&str>,
    ) -> Option<String> {
        if let Some(content) = manifest_content {
            return self.parse_composer_version(content);
        }
        None
    }

    fn parse_composer_version(&self, content: &str) -> Option<String> {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(php_req) = json["require"]["php"].as_str() {
                let ver = php_req
                    .trim()
                    .trim_start_matches(">=")
                    .trim_start_matches("^")
                    .trim_start_matches("~")
                    .split('.')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(".");
                if !ver.is_empty() {
                    return Some(ver);
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

    #[test]
    fn test_find_entrypoint_public_index() {
        let runtime = PhpRuntime;
        let files = vec![
            PathBuf::from("composer.json"),
            PathBuf::from("public/index.php"),
            PathBuf::from("src/Kernel.php"),
        ];

        let entrypoint = runtime.find_entrypoint(&files);
        assert_eq!(
            entrypoint,
            Some("/usr/bin/php -S 0.0.0.0:8000 -t /app/public".to_string())
        );
    }

    #[test]
    fn test_find_entrypoint_none() {
        let runtime = PhpRuntime;
        let files = vec![
            PathBuf::from("composer.json"),
            PathBuf::from("src/Kernel.php"),
        ];

        let entrypoint = runtime.find_entrypoint(&files);
        assert_eq!(entrypoint, None);
    }
}

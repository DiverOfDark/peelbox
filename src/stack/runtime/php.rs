use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct PhpRuntime;

impl Runtime for PhpRuntime {
    fn name(&self) -> &str {
        "PHP"
    }

    fn try_extract(
        &self,
        _files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let port = framework.and_then(|f| f.default_ports().first().copied());
        let health = framework.and_then(|f| {
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint: None,
            port,
            env_vars: vec![],
            health,
            native_deps: vec![],
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("8.2");
        format!("php:{}-fpm-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec![]
    }

    fn start_command(&self, _entrypoint: &Path) -> String {
        "php-fpm".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_php_start_command() {
        let runtime = PhpRuntime;
        let entrypoint = Path::new("index.php");
        assert_eq!(runtime.start_command(entrypoint), "php-fpm");
    }
}

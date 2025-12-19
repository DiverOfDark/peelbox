use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct NativeRuntime;

impl Runtime for NativeRuntime {
    fn name(&self) -> &str {
        "Native"
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

    fn runtime_base_image(&self, _version: Option<&str>) -> String {
        "alpine:latest".to_string()
    }

    fn required_packages(&self) -> Vec<&str> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("./{}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_runtime_name() {
        let runtime = NativeRuntime;
        assert_eq!(runtime.name(), "Native");
    }

    #[test]
    fn test_native_runtime_base_image_default() {
        let runtime = NativeRuntime;
        assert_eq!(runtime.runtime_base_image(None), "alpine:latest");
    }

    #[test]
    fn test_native_required_packages() {
        let runtime = NativeRuntime;
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_native_start_command() {
        let runtime = NativeRuntime;
        let entrypoint = Path::new("app");
        assert_eq!(runtime.start_command(entrypoint), "./app");
    }
}

use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct PythonRuntime;

impl Runtime for PythonRuntime {
    fn name(&self) -> &str {
        "Python"
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
        let version = version.unwrap_or("3.11");
        format!("python:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("python {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_runtime_name() {
        let runtime = PythonRuntime;
        assert_eq!(runtime.name(), "Python");
    }

    #[test]
    fn test_python_runtime_base_image_default() {
        let runtime = PythonRuntime;
        assert_eq!(runtime.runtime_base_image(None), "python:3.11-alpine");
    }

    #[test]
    fn test_python_runtime_base_image_versioned() {
        let runtime = PythonRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("3.12")),
            "python:3.12-alpine"
        );
    }

    #[test]
    fn test_python_required_packages() {
        let runtime = PythonRuntime;
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_python_start_command() {
        let runtime = PythonRuntime;
        let entrypoint = Path::new("main.py");
        assert_eq!(runtime.start_command(entrypoint), "python main.py");
    }
}

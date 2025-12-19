use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct DotNetRuntime;

impl Runtime for DotNetRuntime {
    fn name(&self) -> &str {
        ".NET"
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
        let version = version.unwrap_or("8.0");
        format!("mcr.microsoft.com/dotnet/aspnet:{}", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("dotnet {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dotnet_runtime_name() {
        let runtime = DotNetRuntime;
        assert_eq!(runtime.name(), ".NET");
    }

    #[test]
    fn test_dotnet_runtime_base_image_default() {
        let runtime = DotNetRuntime;
        assert_eq!(
            runtime.runtime_base_image(None),
            "mcr.microsoft.com/dotnet/aspnet:8.0"
        );
    }

    #[test]
    fn test_dotnet_runtime_base_image_versioned() {
        let runtime = DotNetRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("7.0")),
            "mcr.microsoft.com/dotnet/aspnet:7.0"
        );
    }

    #[test]
    fn test_dotnet_required_packages() {
        let runtime = DotNetRuntime;
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_dotnet_start_command() {
        let runtime = DotNetRuntime;
        let entrypoint = Path::new("app.dll");
        assert_eq!(runtime.start_command(entrypoint), "dotnet app.dll");
    }
}

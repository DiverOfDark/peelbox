use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct NodeRuntime;

impl Runtime for NodeRuntime {
    fn name(&self) -> &str {
        "Node"
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
        let version = version.unwrap_or("20");
        format!("node:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec!["dumb-init"]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("node {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_runtime_name() {
        let runtime = NodeRuntime;
        assert_eq!(runtime.name(), "Node");
    }

    #[test]
    fn test_node_runtime_base_image_default() {
        let runtime = NodeRuntime;
        assert_eq!(runtime.runtime_base_image(None), "node:20-alpine");
    }

    #[test]
    fn test_node_runtime_base_image_versioned() {
        let runtime = NodeRuntime;
        assert_eq!(runtime.runtime_base_image(Some("18")), "node:18-alpine");
    }

    #[test]
    fn test_node_required_packages() {
        let runtime = NodeRuntime;
        assert_eq!(runtime.required_packages(), vec!["dumb-init"]);
    }

    #[test]
    fn test_node_start_command() {
        let runtime = NodeRuntime;
        let entrypoint = Path::new("index.js");
        assert_eq!(runtime.start_command(entrypoint), "node index.js");
    }
}

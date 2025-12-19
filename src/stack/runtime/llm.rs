use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct LLMRuntime;

impl Runtime for LLMRuntime {
    fn name(&self) -> &str {
        "LLM"
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
    fn test_llm_runtime_name() {
        let runtime = LLMRuntime;
        assert_eq!(runtime.name(), "LLM");
    }

    #[test]
    fn test_llm_runtime_base_image_default() {
        let runtime = LLMRuntime;
        assert_eq!(runtime.runtime_base_image(None), "alpine:latest");
    }

    #[test]
    fn test_llm_required_packages() {
        let runtime = LLMRuntime;
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_llm_start_command() {
        let runtime = LLMRuntime;
        let entrypoint = Path::new("unknown");
        assert_eq!(runtime.start_command(entrypoint), "./unknown");
    }
}

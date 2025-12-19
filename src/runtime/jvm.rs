use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

pub struct JvmRuntime;

#[async_trait]
impl Runtime for JvmRuntime {
    fn name(&self) -> &str {
        "JVM"
    }

    fn try_deterministic_config(
        &self,
        _files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        // For now, return None - deterministic parsing will be implemented later
        // This allows framework defaults to be used
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

    async fn extract_config_llm(
        &self,
        _files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Result<RuntimeConfig> {
        // LLM extraction not implemented yet - use framework defaults
        Ok(RuntimeConfig {
            entrypoint: Some("app.jar".to_string()),
            port: framework
                .and_then(|f| f.default_ports().first().copied())
                .or(Some(8080)),
            env_vars: vec![],
            health: framework.and_then(|f| {
                f.health_endpoints().first().map(|endpoint| HealthCheck {
                    endpoint: endpoint.to_string(),
                })
            }),
            native_deps: vec![],
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("21");
        format!("eclipse-temurin:{}-jre-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec!["ca-certificates"]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("java -jar {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jvm_runtime_name() {
        let runtime = JvmRuntime;
        assert_eq!(runtime.name(), "JVM");
    }

    #[test]
    fn test_jvm_runtime_base_image_default() {
        let runtime = JvmRuntime;
        assert_eq!(
            runtime.runtime_base_image(None),
            "eclipse-temurin:21-jre-alpine"
        );
    }

    #[test]
    fn test_jvm_runtime_base_image_versioned() {
        let runtime = JvmRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("17")),
            "eclipse-temurin:17-jre-alpine"
        );
    }

    #[test]
    fn test_jvm_required_packages() {
        let runtime = JvmRuntime;
        assert_eq!(runtime.required_packages(), vec!["ca-certificates"]);
    }

    #[test]
    fn test_jvm_start_command() {
        let runtime = JvmRuntime;
        let entrypoint = Path::new("app.jar");
        assert_eq!(runtime.start_command(entrypoint), "java -jar app.jar");
    }
}

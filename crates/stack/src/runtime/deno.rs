use crate::framework::Framework;
use peelbox_core::output::schema::HealthCheck;
use std::path::{Path, PathBuf};

use super::{Runtime, RuntimeConfig};

pub struct DenoRuntime;

impl Runtime for DenoRuntime {
    fn name(&self) -> &str {
        "Deno"
    }

    fn try_extract(
        &self,
        _files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let mut config = RuntimeConfig {
            entrypoint: Some("main.ts".to_string()),
            port: Some(8000),
            env_vars: vec![],
            health: None,
            native_deps: vec![],
        };

        if let Some(fw) = framework {
            if let Some(port) = fw.default_ports().first() {
                config.port = Some(*port);
            }
            if let Some(ep) = fw.entrypoint_command() {
                if let Some(cmd) = ep.last() {
                    config.entrypoint = Some(cmd.clone());
                }
            }
            if let Some(endpoint) = fw.health_endpoints(&[]).first() {
                config.health = Some(HealthCheck {
                    endpoint: format!(
                        "http://localhost:{}{}",
                        config.port.unwrap_or(8000),
                        endpoint
                    ),
                });
            }
        }

        Some(config)
    }

    fn runtime_base_image(&self, _version: Option<&str>) -> String {
        "cgr.dev/chainguard/wolfi-base".to_string()
    }

    fn required_packages(&self) -> Vec<String> {
        vec![] // deno is already in runtime_packages()
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!(
            "deno run --allow-net --allow-read --allow-env {}",
            entrypoint.display()
        )
    }

    fn runtime_packages(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> Vec<String> {
        vec!["deno".to_string()] // ca-certificates is added automatically in assemble phase
    }

    fn runtime_env(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> std::collections::HashMap<String, String> {
        vec![("DENO_DIR".to_string(), "/deno-dir".to_string())]
            .into_iter()
            .collect()
    }
}

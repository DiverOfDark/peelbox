use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct BeamRuntime;

impl Runtime for BeamRuntime {
    fn name(&self) -> &str {
        "BEAM"
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
        let version = version.unwrap_or("1.15");
        format!("hexpm/elixir:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("{} start", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beam_runtime_name() {
        let runtime = BeamRuntime;
        assert_eq!(runtime.name(), "BEAM");
    }

    #[test]
    fn test_beam_runtime_base_image_default() {
        let runtime = BeamRuntime;
        assert_eq!(
            runtime.runtime_base_image(None),
            "hexpm/elixir:1.15-alpine"
        );
    }

    #[test]
    fn test_beam_runtime_base_image_versioned() {
        let runtime = BeamRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("1.16")),
            "hexpm/elixir:1.16-alpine"
        );
    }

    #[test]
    fn test_beam_required_packages() {
        let runtime = BeamRuntime;
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_beam_start_command() {
        let runtime = BeamRuntime;
        let entrypoint = Path::new("_build/prod/rel/app/bin/app");
        assert_eq!(
            runtime.start_command(entrypoint),
            "_build/prod/rel/app/bin/app start"
        );
    }
}

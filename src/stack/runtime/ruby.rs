use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use std::path::{Path, PathBuf};

pub struct RubyRuntime;

impl Runtime for RubyRuntime {
    fn name(&self) -> &str {
        "Ruby"
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
        let version = version.unwrap_or("3.2");
        format!("ruby:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("ruby {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruby_runtime_name() {
        let runtime = RubyRuntime;
        assert_eq!(runtime.name(), "Ruby");
    }

    #[test]
    fn test_ruby_runtime_base_image_default() {
        let runtime = RubyRuntime;
        assert_eq!(runtime.runtime_base_image(None), "ruby:3.2-alpine");
    }

    #[test]
    fn test_ruby_runtime_base_image_versioned() {
        let runtime = RubyRuntime;
        assert_eq!(runtime.runtime_base_image(Some("3.3")), "ruby:3.3-alpine");
    }

    #[test]
    fn test_ruby_required_packages() {
        let runtime = RubyRuntime;
        let packages: Vec<&str> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_ruby_start_command() {
        let runtime = RubyRuntime;
        let entrypoint = Path::new("app.rb");
        assert_eq!(runtime.start_command(entrypoint), "ruby app.rb");
    }
}

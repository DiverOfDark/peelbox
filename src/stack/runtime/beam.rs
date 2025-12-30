use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct BeamRuntime;

impl BeamRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let env_pattern = Regex::new(r#"System\.get_env\("([A-Z_][A-Z0-9_]*)"\)"#).unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "ex" || ext == "exs" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        for cap in env_pattern.captures_iter(&content) {
                            if let Some(var) = cap.get(1) {
                                env_vars.insert(var.as_str().to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut vars: Vec<String> = env_vars.into_iter().collect();
        vars.sort();
        vars
    }

    fn extract_ports(&self, files: &[PathBuf]) -> Option<u16> {
        let cowboy_pattern = Regex::new(r":cowboy.*port:\s*(\d+)").unwrap();
        let ranch_pattern = Regex::new(r":ranch.*port:\s*(\d+)").unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "ex" || ext == "exs" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if let Some(cap) = cowboy_pattern.captures(&content) {
                            if let Some(port_str) = cap.get(1) {
                                if let Ok(port) = port_str.as_str().parse::<u16>() {
                                    return Some(port);
                                }
                            }
                        }
                        if let Some(cap) = ranch_pattern.captures(&content) {
                            if let Some(port_str) = cap.get(1) {
                                if let Ok(port) = port_str.as_str().parse::<u16>() {
                                    return Some(port);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_native_deps(&self, files: &[PathBuf]) -> Vec<String> {
        let mut deps = HashSet::new();

        for file in files {
            if file.file_name().is_some_and(|n| n == "mix.exs") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if content.contains(":nif") || content.contains("rustler") {
                        deps.insert("build-base".to_string());
                    }
                }
            }
        }

        let mut result: Vec<String> = deps.into_iter().collect();
        result.sort();
        result
    }
}

impl Runtime for BeamRuntime {
    fn name(&self) -> &str {
        "BEAM"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let env_vars = self.extract_env_vars(files);
        let native_deps = self.extract_native_deps(files);
        let detected_port = self.extract_ports(files);

        let port =
            detected_port.or_else(|| framework.and_then(|f| f.default_ports().first().copied()));
        let health = framework.and_then(|f| {
            f.health_endpoints(&[]).first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint: None,
            port,
            env_vars,
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("1.15");
        format!("hexpm/elixir:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<String> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("{} start", entrypoint.display())
    }

    fn runtime_packages(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> Vec<String> {
        let available = wolfi_index.get_versions("elixir");

        let version = wolfi_index
            .get_latest_version("elixir")
            .or_else(|| available.first().map(|v| format!("elixir-{}", v)))
            .unwrap_or_else(|| "elixir-1.16".to_string());

        vec![version]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_beam_runtime_name() {
        let runtime = BeamRuntime;
        assert_eq!(runtime.name(), "BEAM");
    }

    #[test]
    fn test_beam_runtime_base_image_default() {
        let runtime = BeamRuntime;
        assert_eq!(runtime.runtime_base_image(None), "hexpm/elixir:1.15-alpine");
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
        let packages: Vec<String> = vec![];
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

    #[test]
    fn test_extract_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let ex_file = temp_dir.path().join("config.ex");
        fs::write(
            &ex_file,
            r#"
db_url = System.get_env("DATABASE_URL")
api_key = System.get_env("API_KEY")
"#,
        )
        .unwrap();

        let runtime = BeamRuntime;
        let files = vec![ex_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL"]);
    }

    #[test]
    fn test_extract_ports_cowboy() {
        let temp_dir = TempDir::new().unwrap();
        let ex_file = temp_dir.path().join("endpoint.ex");
        fs::write(
            &ex_file,
            r#"
:cowboy.start_http(:my_app, 100, port: 4000)
"#,
        )
        .unwrap();

        let runtime = BeamRuntime;
        let files = vec![ex_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(4000));
    }

    #[test]
    fn test_extract_native_deps() {
        let temp_dir = TempDir::new().unwrap();
        let mix_file = temp_dir.path().join("mix.exs");
        fs::write(
            &mix_file,
            r#"
defp deps do
  [
    {:rustler, "~> 0.27.0"}
  ]
end
"#,
        )
        .unwrap();

        let runtime = BeamRuntime;
        let files = vec![mix_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base".to_string()]);
    }
}

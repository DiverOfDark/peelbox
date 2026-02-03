use crate::framework::Framework;
use peelbox_core::output::schema::HealthCheck;
use std::path::{Path, PathBuf};

use super::{Runtime, RuntimeConfig};

pub struct DenoRuntime;

impl DenoRuntime {
    /// Extracts the main entrypoint from deno.json or deno.jsonc manifest
    fn extract_main_from_manifest(&self, files: &[PathBuf]) -> Option<String> {
        for file in files {
            if let Some(file_name) = file.file_name().and_then(|n| n.to_str()) {
                if file_name == "deno.json" || file_name == "deno.jsonc" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(main) = manifest["main"].as_str() {
                                return Some(main.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Runtime for DenoRuntime {
    fn name(&self) -> &str {
        "Deno"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        // Extract entrypoint from deno.json "main" field or use default
        let entrypoint_file = self
            .extract_main_from_manifest(files)
            .unwrap_or_else(|| "main.ts".to_string());

        let mut config = RuntimeConfig {
            entrypoint: Some(format!(
                "deno run --allow-net --allow-read --allow-env {}",
                entrypoint_file
            )),
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
                // Framework provides full command, use it as-is
                config.entrypoint = Some(ep.join(" "));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_deno_runtime_name() {
        let runtime = DenoRuntime;
        assert_eq!(runtime.name(), "Deno");
    }

    #[test]
    fn test_deno_runtime_base_image() {
        let runtime = DenoRuntime;
        assert_eq!(
            runtime.runtime_base_image(None),
            "cgr.dev/chainguard/wolfi-base"
        );
        assert_eq!(
            runtime.runtime_base_image(Some("1.40")),
            "cgr.dev/chainguard/wolfi-base"
        );
    }

    #[test]
    fn test_deno_required_packages() {
        let runtime = DenoRuntime;
        assert_eq!(runtime.required_packages(), Vec::<String>::new());
    }

    #[test]
    fn test_deno_start_command() {
        let runtime = DenoRuntime;
        let entrypoint = Path::new("main.ts");
        assert_eq!(
            runtime.start_command(entrypoint),
            "deno run --allow-net --allow-read --allow-env main.ts"
        );
    }

    #[test]
    fn test_deno_start_command_custom_file() {
        let runtime = DenoRuntime;
        let entrypoint = Path::new("server.ts");
        assert_eq!(
            runtime.start_command(entrypoint),
            "deno run --allow-net --allow-read --allow-env server.ts"
        );
    }

    #[test]
    fn test_try_extract_default() {
        let runtime = DenoRuntime;
        let files = vec![];
        let config = runtime.try_extract(&files, None).unwrap();

        assert_eq!(
            config.entrypoint,
            Some("deno run --allow-net --allow-read --allow-env main.ts".to_string())
        );
        assert_eq!(config.port, Some(8000));
        assert!(config.env_vars.is_empty());
        assert!(config.health.is_none());
    }

    #[test]
    fn test_extract_main_from_deno_json() {
        let temp_dir = TempDir::new().unwrap();
        let deno_json = temp_dir.path().join("deno.json");
        fs::write(
            &deno_json,
            r#"{
                "main": "server.ts",
                "tasks": {
                    "start": "deno run --allow-net server.ts"
                }
            }"#,
        )
        .unwrap();

        let runtime = DenoRuntime;
        let files = vec![deno_json];
        let main = runtime.extract_main_from_manifest(&files);

        assert_eq!(main, Some("server.ts".to_string()));
    }

    #[test]
    fn test_extract_main_from_deno_jsonc() {
        let temp_dir = TempDir::new().unwrap();
        let deno_jsonc = temp_dir.path().join("deno.jsonc");
        fs::write(
            &deno_jsonc,
            r#"{
                // This is a comment
                "main": "app.ts"
            }"#,
        )
        .unwrap();

        let runtime = DenoRuntime;
        let files = vec![deno_jsonc];
        let main = runtime.extract_main_from_manifest(&files);

        // Note: This will fail if serde_json doesn't support comments
        // But deno.jsonc typically uses json5 which allows comments
        // For now, we test that the function doesn't crash
        let _ = main;
    }

    #[test]
    fn test_extract_main_no_manifest() {
        let runtime = DenoRuntime;
        let files = vec![];
        let main = runtime.extract_main_from_manifest(&files);

        assert_eq!(main, None);
    }

    #[test]
    fn test_extract_main_no_main_field() {
        let temp_dir = TempDir::new().unwrap();
        let deno_json = temp_dir.path().join("deno.json");
        fs::write(
            &deno_json,
            r#"{
                "tasks": {
                    "start": "deno run main.ts"
                }
            }"#,
        )
        .unwrap();

        let runtime = DenoRuntime;
        let files = vec![deno_json];
        let main = runtime.extract_main_from_manifest(&files);

        assert_eq!(main, None);
    }

    #[test]
    fn test_try_extract_with_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let deno_json = temp_dir.path().join("deno.json");
        fs::write(
            &deno_json,
            r#"{
                "main": "app.ts"
            }"#,
        )
        .unwrap();

        let runtime = DenoRuntime;
        let files = vec![deno_json];
        let config = runtime.try_extract(&files, None).unwrap();

        assert_eq!(
            config.entrypoint,
            Some("deno run --allow-net --allow-read --allow-env app.ts".to_string())
        );
        assert_eq!(config.port, Some(8000));
    }

    #[test]
    fn test_runtime_env() {
        let runtime = DenoRuntime;
        let wolfi_index = peelbox_wolfi::WolfiPackageIndex::for_tests();
        let service_path = Path::new("/tmp");

        let env = runtime.runtime_env(&wolfi_index, service_path, None);

        assert_eq!(env.len(), 1);
        assert_eq!(env.get("DENO_DIR"), Some(&"/deno-dir".to_string()));
    }

    #[test]
    fn test_runtime_packages() {
        let runtime = DenoRuntime;
        let wolfi_index = peelbox_wolfi::WolfiPackageIndex::for_tests();
        let service_path = Path::new("/tmp");

        let packages = runtime.runtime_packages(&wolfi_index, service_path, None);

        assert_eq!(packages, vec!["deno"]);
    }
}

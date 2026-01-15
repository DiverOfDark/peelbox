use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct NativeRuntime;

impl NativeRuntime {
    fn extract_ports_from_source(&self, files: &[PathBuf]) -> Option<u16> {
        let bind_pattern = Regex::new(r"bind\s*\([^,)]*,\s*[^,)]*,\s*(\d+)\s*\)").unwrap();
        let listen_pattern = Regex::new(r"listen\s*\(\s*(\d+)\s*\)").unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "c" || ext == "cpp" || ext == "cc" || ext == "rs" || ext == "go" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if let Some(cap) = bind_pattern.captures(&content) {
                            if let Some(port_str) = cap.get(1) {
                                if let Ok(port) = port_str.as_str().parse::<u16>() {
                                    return Some(port);
                                }
                            }
                        }
                        if let Some(cap) = listen_pattern.captures(&content) {
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

    fn extract_metadata_hints(&self, files: &[PathBuf]) -> (Option<u16>, Vec<String>) {
        let mut port = None;
        let deps = HashSet::new();

        let cargo_port_pattern = Regex::new(r#"(?m)^#\s*port\s*=\s*(\d+)"#).unwrap();
        let go_port_pattern = Regex::new(r#"(?m)^//\s*port\s*=\s*(\d+)"#).unwrap();

        for file in files {
            if file.file_name().is_some_and(|n| n == "Cargo.toml") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if let Some(cap) = cargo_port_pattern.captures(&content) {
                        if let Some(port_str) = cap.get(1) {
                            if let Ok(p) = port_str.as_str().parse::<u16>() {
                                port = Some(p);
                            }
                        }
                    }
                }
            } else if file.file_name().is_some_and(|n| n == "go.mod") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if let Some(cap) = go_port_pattern.captures(&content) {
                        if let Some(port_str) = cap.get(1) {
                            if let Ok(p) = port_str.as_str().parse::<u16>() {
                                port = Some(p);
                            }
                        }
                    }
                }
            }
        }

        let mut result_deps: Vec<String> = deps.into_iter().collect();
        result_deps.sort();
        (port, result_deps)
    }
}

impl Runtime for NativeRuntime {
    fn name(&self) -> &str {
        "Native"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let source_port = self.extract_ports_from_source(files);
        let (metadata_port, native_deps) = self.extract_metadata_hints(files);

        let detected_port = source_port.or(metadata_port);
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
            env_vars: vec![],
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, _version: Option<&str>) -> String {
        "alpine:latest".to_string()
    }

    fn required_packages(&self) -> Vec<String> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("./{}", entrypoint.display())
    }

    fn runtime_packages(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> Vec<String> {
        vec!["glibc".to_string(), "ca-certificates".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_native_runtime_name() {
        let runtime = NativeRuntime;
        assert_eq!(runtime.name(), "Native");
    }

    #[test]
    fn test_native_runtime_base_image_default() {
        let runtime = NativeRuntime;
        assert_eq!(runtime.runtime_base_image(None), "alpine:latest");
    }

    #[test]
    fn test_native_required_packages() {
        let runtime = NativeRuntime;
        let packages: Vec<String> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_native_start_command() {
        let runtime = NativeRuntime;
        let entrypoint = Path::new("app");
        assert_eq!(runtime.start_command(entrypoint), "./app");
    }

    #[test]
    fn test_extract_ports_from_c_source() {
        let temp_dir = TempDir::new().unwrap();
        let c_file = temp_dir.path().join("server.c");
        fs::write(
            &c_file,
            r#"
#include <sys/socket.h>
int main() {
    bind(sockfd, addr, 8080);
}
"#,
        )
        .unwrap();

        let runtime = NativeRuntime;
        let files = vec![c_file];
        let port = runtime.extract_ports_from_source(&files);

        assert_eq!(port, Some(8080));
    }

    #[test]
    fn test_extract_ports_from_rust_source() {
        let temp_dir = TempDir::new().unwrap();
        let rs_file = temp_dir.path().join("main.rs");
        fs::write(
            &rs_file,
            r#"
fn main() {
    listener.listen(3000);
}
"#,
        )
        .unwrap();

        let runtime = NativeRuntime;
        let files = vec![rs_file];
        let port = runtime.extract_ports_from_source(&files);

        assert_eq!(port, Some(3000));
    }

    #[test]
    fn test_extract_metadata_from_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let cargo_file = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_file,
            r#"
[package]
name = "myapp"
# port = 9000
"#,
        )
        .unwrap();

        let runtime = NativeRuntime;
        let files = vec![cargo_file];
        let (port, _deps) = runtime.extract_metadata_hints(&files);

        assert_eq!(port, Some(9000));
    }

    #[test]
    fn test_extract_metadata_from_go_mod() {
        let temp_dir = TempDir::new().unwrap();
        let go_mod_file = temp_dir.path().join("go.mod");
        fs::write(
            &go_mod_file,
            r#"
module myapp
// port = 8000
"#,
        )
        .unwrap();

        let runtime = NativeRuntime;
        let files = vec![go_mod_file];
        let (port, _deps) = runtime.extract_metadata_hints(&files);

        assert_eq!(port, Some(8000));
    }
}

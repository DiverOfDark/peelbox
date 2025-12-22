use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct NodeRuntime;

impl NodeRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let env_pattern = Regex::new(r"process\.env\.([A-Z_][A-Z0-9_]*)").unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "js" || ext == "ts" || ext == "mjs" || ext == "cjs" {
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
        let listen_pattern = Regex::new(r"\.listen\s*\(\s*(\d+)\s*\)").unwrap();
        let port_arg_pattern = Regex::new(r"--port\s+(\d+)").unwrap();

        for file in files {
            if file.file_name().is_some_and(|n| n == "package.json") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    for cap in port_arg_pattern.captures_iter(&content) {
                        if let Some(port_str) = cap.get(1) {
                            if let Ok(port) = port_str.as_str().parse::<u16>() {
                                return Some(port);
                            }
                        }
                    }
                }
            } else if let Some(ext) = file.extension() {
                if ext == "js" || ext == "ts" || ext == "mjs" || ext == "cjs" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        for cap in listen_pattern.captures_iter(&content) {
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
}

impl Runtime for NodeRuntime {
    fn name(&self) -> &str {
        "Node"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let env_vars = self.extract_env_vars(files);
        let detected_port = self.extract_ports(files);

        let port =
            detected_port.or_else(|| framework.and_then(|f| f.default_ports().first().copied()));
        let health = framework.and_then(|f| {
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint: None,
            port,
            env_vars,
            health,
            native_deps: vec![],
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("20");
        format!("node:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<String> {
        vec!["dumb-init".to_string()]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("node {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
        assert_eq!(runtime.required_packages(), vec!["dumb-init".to_string()]);
    }

    #[test]
    fn test_node_start_command() {
        let runtime = NodeRuntime;
        let entrypoint = Path::new("index.js");
        assert_eq!(runtime.start_command(entrypoint), "node index.js");
    }

    #[test]
    fn test_extract_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let js_file = temp_dir.path().join("server.js");
        fs::write(
            &js_file,
            r#"
            const dbUrl = process.env.DATABASE_URL;
            const apiKey = process.env.API_KEY;
            const port = process.env.PORT;
            "#,
        )
        .unwrap();

        let runtime = NodeRuntime;
        let files = vec![js_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL", "PORT"]);
    }

    #[test]
    fn test_extract_ports_listen() {
        let temp_dir = TempDir::new().unwrap();
        let js_file = temp_dir.path().join("server.js");
        fs::write(
            &js_file,
            r#"
            const express = require('express');
            const app = express();
            app.listen(3000);
            "#,
        )
        .unwrap();

        let runtime = NodeRuntime;
        let files = vec![js_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(3000));
    }

    #[test]
    fn test_extract_ports_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_file = temp_dir.path().join("package.json");
        fs::write(
            &pkg_file,
            r#"
            {
                "scripts": {
                    "start": "node server.js --port 8080"
                }
            }
            "#,
        )
        .unwrap();

        let runtime = NodeRuntime;
        let files = vec![pkg_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(8080));
    }
}

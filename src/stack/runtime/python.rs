use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct PythonRuntime;

impl PythonRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let os_environ_pattern = Regex::new(
            r#"os\.environ(?:\[['"]([A-Z_][A-Z0-9_]*)['"]\]|\.get\(['"]([A-Z_][A-Z0-9_]*)['"])"#,
        )
        .unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "py" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        for cap in os_environ_pattern.captures_iter(&content) {
                            if let Some(var) = cap.get(1).or_else(|| cap.get(2)) {
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
        let app_run_pattern = Regex::new(r"app\.run\s*\([^)]*port\s*=\s*(\d+)").unwrap();
        let listen_pattern = Regex::new(r"\.listen\s*\(\s*(\d+)\s*\)").unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "py" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if let Some(cap) = app_run_pattern.captures(&content) {
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

    fn extract_native_deps(&self, files: &[PathBuf]) -> Vec<String> {
        let mut deps = HashSet::new();

        for file in files {
            if file.file_name().is_some_and(|n| n == "requirements.txt") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    for line in content.lines() {
                        let lower = line.to_lowercase();
                        if lower.contains("numpy")
                            || lower.contains("scipy")
                            || lower.contains("pandas")
                            || lower.contains("pillow")
                            || lower.contains("psycopg")
                            || lower.contains("mysqlclient")
                            || lower.contains("cffi")
                        {
                            deps.insert("build-base".to_string());
                            break;
                        }
                    }
                }
            }
        }

        let mut result: Vec<String> = deps.into_iter().collect();
        result.sort();
        result
    }
}

impl Runtime for PythonRuntime {
    fn name(&self) -> &str {
        "Python"
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
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        let entrypoint = self.detect_framework_entrypoint(files, framework, port);

        Some(RuntimeConfig {
            entrypoint,
            port,
            env_vars,
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("3.11");
        format!("python:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<String> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("python {}", entrypoint.display())
    }

    fn runtime_packages(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> Vec<String> {
        let requested = self.detect_version(service_path, manifest_content);
        let available = wolfi_index.get_versions("python");

        let version = requested
            .as_deref()
            .and_then(|r| wolfi_index.match_version("python", r, &available))
            .or_else(|| wolfi_index.get_latest_version("python"))
            .unwrap_or_else(|| "python-3.12".to_string());

        vec![version]
    }
}

impl PythonRuntime {
    fn detect_framework_entrypoint(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
        port: Option<u16>,
    ) -> Option<String> {
        tracing::debug!("detect_framework_entrypoint called with {} files, framework: {:?}", files.len(), framework.map(|f| f.id()));

        if let Some(fw) = framework {
            match fw.id() {
                crate::stack::FrameworkId::Flask => {
                    tracing::debug!("Flask framework detected, searching for Flask app in {} files", files.len());
                    if let Some(app_file) = self.find_flask_app(files) {
                        tracing::debug!("Found Flask app at: {:?}", app_file);
                        let port_val = port.unwrap_or(5000);
                        let cmd = format!(
                            "python -m flask run --host=0.0.0.0 --port={}",
                            port_val
                        );
                        tracing::debug!("Returning Flask entrypoint: {}", cmd);
                        return Some(cmd);
                    } else {
                        tracing::debug!("No Flask app file found");
                    }
                }
                crate::stack::FrameworkId::Django => {
                    let port_val = port.unwrap_or(8000);
                    return Some(format!(
                        "python manage.py runserver 0.0.0.0:{}",
                        port_val
                    ));
                }
                crate::stack::FrameworkId::FastApi => {
                    let port_val = port.unwrap_or(8000);
                    return Some(format!(
                        "uvicorn main:app --host 0.0.0.0 --port {}",
                        port_val
                    ));
                }
                _ => {}
            }
        } else {
            tracing::debug!("No framework provided");
        }
        None
    }

    fn find_flask_app(&self, files: &[PathBuf]) -> Option<PathBuf> {
        tracing::debug!("find_flask_app: scanning {} files", files.len());
        for file in files {
            tracing::debug!("Checking file: {:?}, ext: {:?}", file, file.extension());
            if let Some(ext) = file.extension() {
                if ext == "py" {
                    tracing::debug!("Reading Python file: {:?}", file);
                    match std::fs::read_to_string(file) {
                        Ok(content) => {
                            let has_flask = content.contains("from flask import") || content.contains("import flask");
                            tracing::debug!("File {:?} has Flask imports: {}", file, has_flask);
                            if has_flask {
                                return Some(file.clone());
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Failed to read {:?}: {}", file, e);
                        }
                    }
                }
            }
        }
        tracing::debug!("No Flask app found in {} files", files.len());
        None
    }

    fn detect_version(&self, service_path: &Path, manifest_content: Option<&str>) -> Option<String> {
        let runtime_txt = service_path.join("runtime.txt");
        if let Ok(content) = std::fs::read_to_string(&runtime_txt) {
            if let Some(ver) = self.normalize_version(&content) {
                return Some(ver);
            }
        }

        let python_version = service_path.join(".python-version");
        if let Ok(content) = std::fs::read_to_string(&python_version) {
            if let Some(ver) = self.normalize_version(&content) {
                return Some(ver);
            }
        }

        if let Some(content) = manifest_content {
            if let Some(ver) = self.parse_pyproject_version(content) {
                return Some(ver);
            }
        }

        None
    }

    fn normalize_version(&self, version_str: &str) -> Option<String> {
        let ver = version_str
            .trim()
            .trim_start_matches(">=")
            .trim_start_matches("^")
            .trim_start_matches("~")
            .trim_start_matches("python")
            .trim()
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");

        if !ver.is_empty() {
            Some(ver)
        } else {
            None
        }
    }

    fn parse_pyproject_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("requires-python") {
                if let Some(eq_pos) = trimmed.find('=') {
                    let value = &trimmed[eq_pos + 1..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    return self.normalize_version(value);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_python_runtime_name() {
        let runtime = PythonRuntime;
        assert_eq!(runtime.name(), "Python");
    }

    #[test]
    fn test_python_runtime_base_image_default() {
        let runtime = PythonRuntime;
        assert_eq!(runtime.runtime_base_image(None), "python:3.11-alpine");
    }

    #[test]
    fn test_python_runtime_base_image_versioned() {
        let runtime = PythonRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("3.12")),
            "python:3.12-alpine"
        );
    }

    #[test]
    fn test_python_required_packages() {
        let runtime = PythonRuntime;
        let packages: Vec<String> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_python_start_command() {
        let runtime = PythonRuntime;
        let entrypoint = Path::new("main.py");
        assert_eq!(runtime.start_command(entrypoint), "python main.py");
    }

    #[test]
    fn test_extract_env_vars_environ() {
        let temp_dir = TempDir::new().unwrap();
        let py_file = temp_dir.path().join("app.py");
        fs::write(
            &py_file,
            r#"
import os
db_url = os.environ['DATABASE_URL']
api_key = os.environ.get('API_KEY')
"#,
        )
        .unwrap();

        let runtime = PythonRuntime;
        let files = vec![py_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL"]);
    }

    #[test]
    fn test_extract_ports_app_run() {
        let temp_dir = TempDir::new().unwrap();
        let py_file = temp_dir.path().join("app.py");
        fs::write(
            &py_file,
            r#"
from flask import Flask
app = Flask(__name__)
if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
"#,
        )
        .unwrap();

        let runtime = PythonRuntime;
        let files = vec![py_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(5000));
    }

    #[test]
    fn test_extract_ports_listen() {
        let temp_dir = TempDir::new().unwrap();
        let py_file = temp_dir.path().join("server.py");
        fs::write(
            &py_file,
            r#"
server = Server()
server.listen(8000)
"#,
        )
        .unwrap();

        let runtime = PythonRuntime;
        let files = vec![py_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(8000));
    }

    #[test]
    fn test_extract_native_deps() {
        let temp_dir = TempDir::new().unwrap();
        let req_file = temp_dir.path().join("requirements.txt");
        fs::write(
            &req_file,
            r#"
Flask==2.0.0
numpy==1.24.0
requests==2.28.0
"#,
        )
        .unwrap();

        let runtime = PythonRuntime;
        let files = vec![req_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base".to_string()]);
    }

    #[test]
    fn test_extract_native_deps_no_native() {
        let temp_dir = TempDir::new().unwrap();
        let req_file = temp_dir.path().join("requirements.txt");
        fs::write(
            &req_file,
            r#"
Flask==2.0.0
requests==2.28.0
"#,
        )
        .unwrap();

        let runtime = PythonRuntime;
        let files = vec![req_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, Vec::<String>::new());
    }
}

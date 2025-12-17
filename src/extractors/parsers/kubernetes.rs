//! Kubernetes manifest parsing utilities

use crate::extractors::env_vars::{EnvVarInfo, EnvVarSource};
use crate::extractors::health::{HealthCheckInfo, HealthCheckSource};
use crate::fs::FileSystem;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Parse environment variables from Kubernetes deployment manifests
pub fn parse_env_vars<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    env_vars: &mut HashMap<String, EnvVarInfo>,
) {
    for k8s_file in &[
        "deployment.yaml",
        "k8s/deployment.yaml",
        "deploy/deployment.yaml",
    ] {
        if let Ok(content) = fs.read_to_string(&service_path.join(k8s_file)) {
            parse_env_vars_from_content(&content, env_vars, k8s_file);
        }
    }
}

fn parse_env_vars_from_content(
    content: &str,
    env_vars: &mut HashMap<String, EnvVarInfo>,
    filename: &str,
) {
    let env_re = Regex::new(r"(?m)^\s*-\s*name:\s*([A-Z_][A-Z0-9_]*)").expect("valid regex");

    for cap in env_re.captures_iter(content) {
        if let Some(name_match) = cap.get(1) {
            let name = name_match.as_str().to_string();
            env_vars.entry(name.clone()).or_insert(EnvVarInfo {
                name,
                default_value: None,
                source: EnvVarSource::ConfigFile(filename.to_string()),
                required: false,
            });
        }
    }
}

/// Parse health check probes from Kubernetes deployment manifests
pub fn parse_health_checks<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    health_checks: &mut Vec<HealthCheckInfo>,
    seen: &mut HashSet<String>,
) {
    for k8s_file in &[
        "deployment.yaml",
        "k8s/deployment.yaml",
        "deploy/deployment.yaml",
    ] {
        if let Ok(content) = fs.read_to_string(&service_path.join(k8s_file)) {
            parse_health_checks_from_content(&content, health_checks, seen, k8s_file);
        }
    }
}

fn parse_health_checks_from_content(
    content: &str,
    health_checks: &mut Vec<HealthCheckInfo>,
    seen: &mut HashSet<String>,
    filename: &str,
) {
    let probe_re = Regex::new(r#"(?m)^\s*path:\s*([/\w\-]+)"#).expect("valid regex");

    for cap in probe_re.captures_iter(content) {
        if let Some(path_match) = cap.get(1) {
            let endpoint = path_match.as_str().to_string();
            if seen.insert(endpoint.clone()) {
                health_checks.push(HealthCheckInfo {
                    endpoint,
                    source: HealthCheckSource::KubernetesManifest(filename.to_string()),
                    confidence: 0.95,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::MockFileSystem;
    use std::path::PathBuf;

    #[test]
    fn test_parse_env_vars() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "deployment.yaml",
            r#"
apiVersion: apps/v1
kind: Deployment
spec:
  template:
    spec:
      containers:
      - env:
        - name: DATABASE_URL
        - name: API_KEY
"#,
        );

        let mut env_vars = HashMap::new();
        parse_env_vars(&PathBuf::from("."), &fs, &mut env_vars);

        assert_eq!(env_vars.len(), 2);
        assert!(env_vars.contains_key("DATABASE_URL"));
        assert!(env_vars.contains_key("API_KEY"));
    }

    #[test]
    fn test_parse_health_checks() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "deployment.yaml",
            r#"
apiVersion: apps/v1
kind: Deployment
spec:
  template:
    spec:
      containers:
      - livenessProbe:
          httpGet:
            path: /healthz
            port: 8080
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
"#,
        );

        let mut health_checks = Vec::new();
        let mut seen = HashSet::new();
        parse_health_checks(&PathBuf::from("."), &fs, &mut health_checks, &mut seen);

        assert_eq!(health_checks.len(), 2);
        assert!(health_checks.iter().any(|h| h.endpoint == "/healthz"));
        assert!(health_checks.iter().any(|h| h.endpoint == "/ready"));
    }

    #[test]
    fn test_no_k8s_files() {
        let fs = MockFileSystem::new();
        let mut env_vars = HashMap::new();

        parse_env_vars(&PathBuf::from("."), &fs, &mut env_vars);
        assert_eq!(env_vars.len(), 0);

        let mut health_checks = Vec::new();
        let mut seen = HashSet::new();
        parse_health_checks(&PathBuf::from("."), &fs, &mut health_checks, &mut seen);
        assert_eq!(health_checks.len(), 0);
    }
}

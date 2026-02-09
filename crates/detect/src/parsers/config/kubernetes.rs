use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

pub struct KubernetesParser;

impl ConfigParser for KubernetesParser {
    fn filenames(&self) -> &[&str] {
        &["deployment.yaml", "deployment.yml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        // Guard: only process Kubernetes manifests
        if !content.contains("apiVersion:") || !content.contains("kind:") {
            return None;
        }

        let mut env_vars = BTreeMap::new();
        let mut ports = Vec::new();
        let mut health_endpoint = None;

        // Extract env var names (e.g., "- name: DATABASE_URL")
        let env_re = regex::Regex::new(r"(?m)^\s*-\s*name:\s*([A-Z_][A-Z0-9_]*)").ok()?;
        for cap in env_re.captures_iter(content) {
            if let Some(name) = cap.get(1) {
                env_vars.insert(name.as_str().to_string(), String::new());
            }
        }

        // Extract health probe paths
        let probe_re = regex::Regex::new(r"(?m)^\s*path:\s*([/\w\-]+)").ok()?;
        if let Some(cap) = probe_re.captures(content) {
            if let Some(path_match) = cap.get(1) {
                health_endpoint = Some(path_match.as_str().to_string());
            }
        }

        // Extract container ports (may appear as "containerPort: 8080" or "- containerPort: 8080")
        let port_re = regex::Regex::new(r"(?m)^\s*-?\s*containerPort:\s*(\d+)").ok()?;
        for cap in port_re.captures_iter(content) {
            if let Some(port_match) = cap.get(1) {
                if let Ok(port) = port_match.as_str().parse::<u16>() {
                    if !ports.contains(&port) {
                        ports.push(port);
                    }
                }
            }
        }

        if env_vars.is_empty() && ports.is_empty() && health_endpoint.is_none() {
            return None;
        }

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars,
            ports,
            health_endpoint,
        })
    }
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(KubernetesParser))
}

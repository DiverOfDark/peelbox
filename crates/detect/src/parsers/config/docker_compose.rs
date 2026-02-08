use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

pub struct DockerComposeParser;

impl ConfigParser for DockerComposeParser {
    fn filenames(&self) -> &[&str] {
        &[
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        let mut ports = Vec::new();
        let mut env_vars = BTreeMap::new();

        // Simple line-based parsing for ports and environment
        let port_re = regex::Regex::new(r#"["']?(\d+):(\d+)["']?"#).ok()?;
        let env_re = regex::Regex::new(r"^\s*-\s*(\w+)=(.+)$").ok()?;

        for line in content.lines() {
            let trimmed = line.trim();

            // Parse port mappings like "8080:8080" or "3000:3000"
            if let Some(caps) = port_re.captures(trimmed) {
                if let Some(port_str) = caps.get(1) {
                    if let Ok(port) = port_str.as_str().parse::<u16>() {
                        if !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }

            // Parse environment variables
            if let Some(caps) = env_re.captures(trimmed) {
                if let (Some(key), Some(val)) = (caps.get(1), caps.get(2)) {
                    env_vars.insert(key.as_str().to_string(), val.as_str().to_string());
                }
            }
        }

        if ports.is_empty() && env_vars.is_empty() {
            return None;
        }

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars,
            ports,
            health_endpoint: None,
        })
    }
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(DockerComposeParser))
}

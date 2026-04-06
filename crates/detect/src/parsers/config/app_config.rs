use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

pub struct AppConfigParser;

impl ConfigParser for AppConfigParser {
    fn filenames(&self) -> &[&str] {
        &[
            "application.yml",
            "application.yaml",
            "application.properties",
            "appsettings.json",
        ]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let mut ports = Vec::new();

        if filename.ends_with(".properties") {
            // Java properties: server.port=8080 or server.port=${PORT:5000}
            let re = regex::Regex::new(r"(?m)^server\.port\s*=\s*(.+)$").ok()?;
            for cap in re.captures_iter(content) {
                if let Some(val) = cap.get(1) {
                    let val = val.as_str().trim();
                    // Try plain port number first
                    if let Ok(port) = val.parse::<u16>() {
                        if !ports.contains(&port) {
                            ports.push(port);
                        }
                    } else if let Some(default) = extract_spring_placeholder_default(val) {
                        if let Ok(port) = default.parse::<u16>() {
                            if !ports.contains(&port) {
                                ports.push(port);
                            }
                        }
                    }
                }
            }
        } else if filename.ends_with(".json") {
            // JSON: "Port": 5000 or "port": 5000
            let re = regex::Regex::new(r#""[Pp]ort"\s*:\s*(\d+)"#).ok()?;
            for cap in re.captures_iter(content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }
        } else {
            // YAML: port: 8080
            let re = regex::Regex::new(r"(?m)^\s*port:\s*(\d+)").ok()?;
            for cap in re.captures_iter(content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }
        }

        if ports.is_empty() {
            return None;
        }

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars: BTreeMap::new(),
            ports,
            health_endpoint: None,
            runtime_command: None,
        })
    }
}

/// Extract the default value from a Spring placeholder like `${PORT:5000}`.
fn extract_spring_placeholder_default(value: &str) -> Option<&str> {
    let inner = value.strip_prefix("${")?.strip_suffix('}')?;
    inner.split_once(':').map(|(_, default)| default.trim())
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(AppConfigParser))
}

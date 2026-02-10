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
            // Java properties: server.port=8080
            let re = regex::Regex::new(r"(?m)^server\.port\s*=\s*(\d+)").ok()?;
            for cap in re.captures_iter(content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if !ports.contains(&port) {
                            ports.push(port);
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
        })
    }
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(AppConfigParser))
}

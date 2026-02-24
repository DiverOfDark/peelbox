use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

pub struct DockerfileParser;

impl ConfigParser for DockerfileParser {
    fn filenames(&self) -> &[&str] {
        &["Dockerfile", "Dockerfile.production"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        let mut ports = Vec::new();
        let mut env_vars = BTreeMap::new();
        let mut health_endpoint = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // EXPOSE ports
            if let Some(rest) = trimmed.strip_prefix("EXPOSE ") {
                for part in rest.split_whitespace() {
                    let port_str = part.split('/').next().unwrap_or(part);
                    if let Ok(port) = port_str.parse::<u16>() {
                        if !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }

            // ENV vars
            if let Some(rest) = trimmed.strip_prefix("ENV ") {
                if let Some((key, value)) = rest.split_once('=') {
                    env_vars.insert(
                        key.trim().to_string(),
                        value.trim().trim_matches('"').to_string(),
                    );
                } else {
                    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        env_vars.insert(parts[0].to_string(), parts[1].to_string());
                    }
                }
            }

            // HEALTHCHECK
            if trimmed.starts_with("HEALTHCHECK") {
                if let Some(idx) = trimmed.find("curl") {
                    let rest = &trimmed[idx..];
                    if let Some(url_start) = rest.find("http") {
                        let url = rest[url_start..].split_whitespace().next().unwrap_or("");
                        if let Some(path_start) = url.find("localhost") {
                            let after_host = &url[path_start..];
                            if let Some(slash_idx) = after_host[9..].find('/') {
                                let endpoint = &after_host[9 + slash_idx..];
                                let endpoint = endpoint
                                    .split(&[' ', '"', '\'', '|', ';'][..])
                                    .next()
                                    .unwrap_or(endpoint);
                                health_endpoint = Some(endpoint.to_string());
                            }
                        }
                    }
                }
            }
        }

        if ports.is_empty() && env_vars.is_empty() && health_endpoint.is_none() {
            return None;
        }

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars,
            ports,
            health_endpoint,
            runtime_command: None,
        })
    }
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(DockerfileParser))
}

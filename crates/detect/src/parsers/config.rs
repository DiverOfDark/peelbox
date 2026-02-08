//! Config file parsers that produce `ConfigContribution`.

use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

// ── EnvFileParser ───────────────────────────────────────────────────────────

pub struct EnvFileParser;

impl ConfigParser for EnvFileParser {
    fn filenames(&self) -> &[&str] {
        &[
            ".env",
            ".env.production",
            ".env.local",
            ".env.example",
            ".env.template",
            ".env.sample",
        ]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        let env_vars: BTreeMap<String, String> = content
            .lines()
            .filter(|line| !line.starts_with('#') && line.contains('='))
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                Some((key.trim().to_string(), value.trim().to_string()))
            })
            .collect();

        if env_vars.is_empty() {
            return None;
        }

        let ports = env_vars
            .iter()
            .filter(|(k, _)| k.contains("PORT"))
            .filter_map(|(_, v)| v.parse::<u16>().ok())
            .collect();

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars,
            ports,
            health_endpoint: None,
        })
    }
}

// ── DockerComposeParser ─────────────────────────────────────────────────────

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

// ── DockerfileParser ────────────────────────────────────────────────────────

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
                        let url = rest[url_start..]
                            .split_whitespace()
                            .next()
                            .unwrap_or("");
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_file_parser() {
        let parser = EnvFileParser;
        let content = "# Comment\nPORT=8080\nDATABASE_URL=postgres://localhost\nAPI_KEY=secret";
        let contrib = parser.parse(Path::new(".env"), content).unwrap();
        assert_eq!(contrib.env_vars.len(), 3);
        assert_eq!(contrib.ports, vec![8080]);
    }

    #[test]
    fn test_dockerfile_parser() {
        let parser = DockerfileParser;
        let content = "FROM node:18\nEXPOSE 3000\nENV NODE_ENV=production\nCMD [\"node\", \"index.js\"]";
        let contrib = parser.parse(Path::new("Dockerfile"), content).unwrap();
        assert_eq!(contrib.ports, vec![3000]);
        assert_eq!(
            contrib.env_vars.get("NODE_ENV"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_docker_compose_parser() {
        let parser = DockerComposeParser;
        let content = r#"
services:
  web:
    ports:
      - "3000:3000"
    environment:
      - NODE_ENV=production
"#;
        let contrib = parser
            .parse(Path::new("docker-compose.yml"), content)
            .unwrap();
        assert_eq!(contrib.ports, vec![3000]);
    }
}

//! Dockerfile parsing utilities

use crate::extractors::health::{HealthCheckInfo, HealthCheckSource};
use crate::extractors::port::{PortInfo, PortSource};
use crate::fs::FileSystem;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

/// Parse EXPOSE directives from Dockerfile
pub fn parse_expose<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    seen: &mut HashSet<u16>,
) -> Vec<PortInfo> {
    let mut ports = Vec::new();

    if let Ok(content) = fs.read_to_string(&service_path.join("Dockerfile")) {
        let expose_re = Regex::new(r"(?m)^EXPOSE\s+(\d+)").expect("valid regex");

        for cap in expose_re.captures_iter(&content) {
            if let Some(port_match) = cap.get(1) {
                if let Ok(port) = port_match.as_str().parse::<u16>() {
                    if seen.insert(port) {
                        ports.push(PortInfo {
                            port,
                            source: PortSource::Dockerfile,
                            confidence: 1.0,
                        });
                    }
                }
            }
        }
    }

    ports
}

/// Parse HEALTHCHECK directives from Dockerfile
pub fn parse_healthcheck<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    seen: &mut HashSet<String>,
) -> Vec<HealthCheckInfo> {
    let mut health_checks = Vec::new();

    if let Ok(content) = fs.read_to_string(&service_path.join("Dockerfile")) {
        let healthcheck_re =
            Regex::new(r#"(?m)^HEALTHCHECK\s+.*curl\s+.*?https?://[^/]+(/[\w\-/]*)"#)
                .expect("valid regex");

        for cap in healthcheck_re.captures_iter(&content) {
            if let Some(endpoint_match) = cap.get(1) {
                let endpoint = endpoint_match.as_str().to_string();
                if seen.insert(endpoint.clone()) {
                    health_checks.push(HealthCheckInfo {
                        endpoint,
                        source: HealthCheckSource::Dockerfile,
                        confidence: 1.0,
                    });
                }
            }
        }
    }

    health_checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::MockFileSystem;
    use std::path::PathBuf;

    #[test]
    fn test_parse_expose() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "Dockerfile",
            r#"
FROM node:20
EXPOSE 3000
EXPOSE 8080
CMD ["node", "server.js"]
"#,
        );

        let mut seen = HashSet::new();
        let ports = parse_expose(&PathBuf::from("."), &fs, &mut seen);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 3000));
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().all(|p| p.source == PortSource::Dockerfile));
    }

    #[test]
    fn test_no_dockerfile() {
        let fs = MockFileSystem::new();
        let mut seen = HashSet::new();

        let ports = parse_expose(&PathBuf::from("."), &fs, &mut seen);
        assert_eq!(ports.len(), 0);
    }
}

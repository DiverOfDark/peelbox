//! .env file parsing utilities

use crate::extractors::port::{PortInfo, PortSource};
use crate::fs::FileSystem;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

/// Parse port variables from .env files (.env.example, .env.template, .env.sample)
pub fn parse_ports<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    seen: &mut HashSet<u16>,
) -> Vec<PortInfo> {
    let mut ports = Vec::new();
    let port_re = Regex::new(r"(?m)^[A-Z_]*PORT[A-Z_]*=(\d+)").expect("valid regex");

    for env_file in &[".env.example", ".env.template", ".env.sample"] {
        if let Ok(content) = fs.read_to_string(&service_path.join(env_file)) {
            for cap in port_re.captures_iter(&content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if seen.insert(port) {
                            ports.push(PortInfo {
                                port,
                                source: PortSource::EnvFile,
                                confidence: 0.9,
                            });
                        }
                    }
                }
            }
        }
    }

    ports
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::MockFileSystem;
    use std::path::PathBuf;

    #[test]
    fn test_parse_ports() {
        let fs = MockFileSystem::new();
        fs.add_file(
            ".env.example",
            r#"
DATABASE_URL=postgres://localhost:5432
PORT=8080
SERVER_PORT=3000
"#,
        );

        let mut seen = HashSet::new();
        let ports = parse_ports(&PathBuf::from("."), &fs, &mut seen);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 3000));
        assert!(ports.iter().all(|p| p.source == PortSource::EnvFile));
    }

    #[test]
    fn test_multiple_env_files() {
        let fs = MockFileSystem::new();
        fs.add_file(".env.example", "PORT=8080\n");
        fs.add_file(".env.template", "API_PORT=9000\n");
        fs.add_file(".env.sample", "ADMIN_PORT=7000\n");

        let mut seen = HashSet::new();
        let ports = parse_ports(&PathBuf::from("."), &fs, &mut seen);

        assert_eq!(ports.len(), 3);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 9000));
        assert!(ports.iter().any(|p| p.port == 7000));
    }

    #[test]
    fn test_no_env_files() {
        let fs = MockFileSystem::new();
        let mut seen = HashSet::new();

        let ports = parse_ports(&PathBuf::from("."), &fs, &mut seen);
        assert_eq!(ports.len(), 0);
    }
}

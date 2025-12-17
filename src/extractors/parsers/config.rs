//! YAML and JSON configuration file parsing utilities

use crate::extractors::port::{PortInfo, PortSource};
use crate::fs::FileSystem;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

/// Parse ports from YAML configuration files (application.yml, application.yaml)
pub fn parse_yaml_ports<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    seen: &mut HashSet<u16>,
) -> Vec<PortInfo> {
    let mut ports = Vec::new();
    let port_re = Regex::new(r"(?m)^\s*port:\s*(\d+)").expect("valid regex");

    for config_file in &[
        "application.yml",
        "application.yaml",
        "src/main/resources/application.yml",
    ] {
        if let Ok(content) = fs.read_to_string(&service_path.join(config_file)) {
            for cap in port_re.captures_iter(&content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if seen.insert(port) {
                            ports.push(PortInfo {
                                port,
                                source: PortSource::ConfigFile(config_file.to_string()),
                                confidence: 0.95,
                            });
                        }
                    }
                }
            }
        }
    }

    ports
}

/// Parse ports from JSON configuration files
pub fn parse_json_ports<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    seen: &mut HashSet<u16>,
) -> Vec<PortInfo> {
    let mut ports = Vec::new();
    let port_re = Regex::new(r#""[Pp]ort"\s*:\s*(\d+)"#).expect("valid regex");

    for config_file in &["config.json", "config/default.json", "appsettings.json"] {
        if let Ok(content) = fs.read_to_string(&service_path.join(config_file)) {
            for cap in port_re.captures_iter(&content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if seen.insert(port) {
                            ports.push(PortInfo {
                                port,
                                source: PortSource::ConfigFile(config_file.to_string()),
                                confidence: 0.95,
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
    fn test_parse_yaml_ports() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "application.yml",
            r#"
server:
  port: 8080
management:
  port: 8081
"#,
        );

        let mut seen = HashSet::new();
        let ports = parse_yaml_ports(&PathBuf::from("."), &fs, &mut seen);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 8081));
    }

    #[test]
    fn test_parse_json_ports() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "config.json",
            r#"
{
  "port": 3000,
  "database": {
    "port": 5432
  }
}
"#,
        );

        let mut seen = HashSet::new();
        let ports = parse_json_ports(&PathBuf::from("."), &fs, &mut seen);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 3000));
        assert!(ports.iter().any(|p| p.port == 5432));
    }

    #[test]
    fn test_no_config_files() {
        let fs = MockFileSystem::new();
        let mut seen = HashSet::new();

        let ports = parse_yaml_ports(&PathBuf::from("."), &fs, &mut seen);
        assert_eq!(ports.len(), 0);

        let ports = parse_json_ports(&PathBuf::from("."), &fs, &mut HashSet::new());
        assert_eq!(ports.len(), 0);
    }
}

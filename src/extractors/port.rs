//! Port extractor - deterministic extraction of port numbers from code and config files

use crate::extractors::{parsers, ServiceContext};
use crate::fs::FileSystem;
use crate::stack::registry::StackRegistry;
use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct PortInfo {
    pub port: u16,
    pub source: PortSource,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortSource {
    Dockerfile,
    EnvFile,
    ConfigFile(String),
    CodePattern(String),
}

pub struct PortExtractor<F: FileSystem> {
    fs: F,
    registry: StackRegistry,
}

impl<F: FileSystem> PortExtractor<F> {
    pub fn new(fs: F) -> Self {
        Self {
            fs,
            registry: StackRegistry::with_defaults(),
        }
    }

    pub fn extract(&self, context: &ServiceContext) -> Vec<PortInfo> {
        let mut ports = Vec::new();
        let mut seen = HashSet::new();

        // Extract from cross-language sources using shared parsers
        ports.extend(parsers::dockerfile::parse_expose(
            &context.path,
            &self.fs,
            &mut seen,
        ));
        ports.extend(parsers::env_file::parse_ports(
            &context.path,
            &self.fs,
            &mut seen,
        ));
        ports.extend(parsers::config::parse_yaml_ports(
            &context.path,
            &self.fs,
            &mut seen,
        ));
        ports.extend(parsers::config::parse_json_ports(
            &context.path,
            &self.fs,
            &mut seen,
        ));

        // Extract from language-specific code patterns
        self.extract_from_code_patterns(context, &mut ports, &mut seen);

        ports
    }

    fn extract_from_code_patterns(
        &self,
        context: &ServiceContext,
        ports: &mut Vec<PortInfo>,
        seen: &mut HashSet<u16>,
    ) {
        let lang = match context
            .language
            .as_ref()
            .and_then(|id| self.registry.get_language(id.clone()))
        {
            Some(l) => l,
            None => return,
        };

        let patterns = lang.port_patterns();
        if patterns.is_empty() {
            return;
        }

        let dir_path = &context.path;
        crate::extractors::common::scan_directory_with_language_filter(
            &self.fs,
            dir_path,
            lang,
            |file_path| {
                self.extract_ports_from_file(file_path, &patterns, ports, seen);
            },
        );
    }

    fn extract_ports_from_file(
        &self,
        file_path: &std::path::Path,
        patterns: &[(&str, &str)],
        ports: &mut Vec<PortInfo>,
        seen: &mut HashSet<u16>,
    ) {
        let content = match self.fs.read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        for (pattern, pattern_name) in patterns {
            let re = match Regex::new(pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for cap in re.captures_iter(&content) {
                if let Some(port) = self.parse_port_from_capture(&cap) {
                    if port >= 1024 && seen.insert(port) {
                        ports.push(PortInfo {
                            port,
                            source: PortSource::CodePattern(pattern_name.to_string()),
                            confidence: 0.8,
                        });
                    }
                }
            }
        }
    }

    fn parse_port_from_capture(&self, cap: &regex::Captures) -> Option<u16> {
        cap.get(1)?.as_str().parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::MockFileSystem;
    use std::path::PathBuf;

    #[test]
    fn test_extract_from_dockerfile() {
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

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 3000));
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().all(|p| p.source == PortSource::Dockerfile));
    }

    #[test]
    fn test_extract_from_env_file() {
        let fs = MockFileSystem::new();
        fs.add_file(
            ".env.example",
            r#"
DATABASE_URL=postgres://localhost:5432
PORT=8080
SERVER_PORT=3000
"#,
        );

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 3000));
    }

    #[test]
    fn test_extract_from_yaml_config() {
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

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 8081));
    }

    #[test]
    fn test_extract_from_json_config() {
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

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 3000));
        assert!(ports.iter().any(|p| p.port == 5432));
    }

    #[test]
    fn test_extract_from_code_listen_pattern() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "server.js",
            r#"
const express = require('express');
const app = express();

app.listen(3000, () => {
  console.log('Server running on port 3000');
});
"#,
        );

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::with_detection(
            PathBuf::from("."),
            Some(crate::stack::LanguageId::JavaScript),
            None,
        );
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3000);
        assert!(matches!(ports[0].source, PortSource::CodePattern(_)));
    }

    #[test]
    fn test_deduplication() {
        let fs = MockFileSystem::new();
        fs.add_file("Dockerfile", "EXPOSE 3000");
        fs.add_file(".env.example", "PORT=3000");
        fs.add_file("application.yml", "server:\n  port: 3000");

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3000);
        assert_eq!(ports[0].source, PortSource::Dockerfile);
    }

    #[test]
    fn test_no_ports_found() {
        let fs = MockFileSystem::new();
        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 0);
    }

    #[test]
    fn test_invalid_port_ignored() {
        let fs = MockFileSystem::new();
        fs.add_file("Dockerfile", "EXPOSE 999999");
        fs.add_file(".env.example", "PORT=abc");

        let extractor = PortExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let ports = extractor.extract(&context);

        assert_eq!(ports.len(), 0);
    }
}

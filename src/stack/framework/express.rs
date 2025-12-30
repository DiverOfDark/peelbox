//! Express framework for JavaScript/TypeScript

use super::*;

pub struct ExpressFramework;

impl Framework for ExpressFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Express
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["JavaScript".to_string(), "TypeScript".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["npm".to_string(), "yarn".to_string(), "pnpm".to_string(), "bun".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::NpmPackage,
            pattern: "express".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![3000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string(), "/ping".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"PORT\s*=\s*(\d+)".to_string(), "Express port".to_string()),
            (r"NODE_ENV\s*=\s*(\w+)".to_string(), "Node environment".to_string()),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec![
            "server.js",
            "app.js",
            "index.js",
            "src/server.js",
            "src/app.js",
            "src/index.js",
        ]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if (trimmed.contains("app.listen") || trimmed.contains("server.listen"))
                && config.port.is_none()
            {
                if let Some(port) = extract_listen_port(trimmed) {
                    config.port = Some(port);
                }
            }

            if (trimmed.contains("PORT") || trimmed.contains("port"))
                && trimmed.contains("||")
                && config.port.is_none()
            {
                if let Some(port) = extract_default_port(trimmed) {
                    config.port = Some(port);
                }
            }

            if trimmed.contains("process.env.") {
                extract_js_env_vars(trimmed, &mut config.env_vars);
            }
        }

        if config.port.is_some() || !config.env_vars.is_empty() {
            Some(config)
        } else {
            None
        }
    }
}

fn extract_listen_port(line: &str) -> Option<u16> {
    if let Some(paren_pos) = line.find('(') {
        let rest = &line[paren_pos + 1..];

        let num_str: String = rest
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_numeric())
            .collect();

        if !num_str.is_empty() {
            return num_str.parse::<u16>().ok();
        }
    }
    None
}

fn extract_default_port(line: &str) -> Option<u16> {
    if let Some(or_pos) = line.find("||") {
        let rest = &line[or_pos + 2..];

        let num_str: String = rest
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_numeric())
            .collect();

        if !num_str.is_empty() {
            return num_str.parse::<u16>().ok();
        }
    }
    None
}

fn extract_js_env_vars(line: &str, env_vars: &mut Vec<String>) {
    let mut pos = 0;
    while let Some(start) = line[pos..].find("process.env.") {
        let abs_start = pos + start + 12;
        let rest = &line[abs_start..];

        let var_name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        if !var_name.is_empty() && !env_vars.contains(&var_name) {
            env_vars.push(var_name.clone());
        }

        pos = abs_start + var_name.len().max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_express_compatibility() {
        let framework = ExpressFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "JavaScript"));
        assert!(framework.compatible_languages().iter().any(|s| s == "TypeScript"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "npm"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "yarn"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "pnpm"));
    }

    #[test]
    fn test_express_dependency_detection() {
        let framework = ExpressFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "express".to_string(),
            version: Some("4.18.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_express_health_endpoints() {
        let framework = ExpressFramework;
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/healthz"));
    }

    #[test]
    fn test_express_default_ports() {
        let framework = ExpressFramework;
        assert_eq!(framework.default_ports(), vec![3000]);
    }

    #[test]
    fn test_express_parse_server() {
        let framework = ExpressFramework;
        let content = r#"
const express = require('express');
const app = express();

const PORT = process.env.PORT || 3001;
const API_KEY = process.env.API_KEY;

app.listen(PORT, () => {
  console.log(`Server running on port ${PORT}`);
});
"#;

        let config = framework
            .parse_config(Path::new("server.js"), content)
            .unwrap();

        assert_eq!(config.port, Some(3001));
        assert!(config.env_vars.contains(&"PORT".to_string()));
        assert!(config.env_vars.contains(&"API_KEY".to_string()));
    }

    #[test]
    fn test_express_config_files() {
        let framework = ExpressFramework;
        let files = framework.config_files();

        assert!(files.iter().any(|s| *s == "server.js"));
        assert!(files.iter().any(|s| *s == "app.js"));
        assert!(files.iter().any(|s| *s == "index.js"));
    }
}

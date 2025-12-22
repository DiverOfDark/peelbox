//! Ruby on Rails framework

use super::*;

pub struct RailsFramework;

impl Framework for RailsFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Rails
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Ruby".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["bundler".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"^rails$".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> &[u16] {
        &[3000]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string(), "/up".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"RAILS_ENV\s*=\s*(\w+)".to_string(), "Rails environment".to_string()),
            (r"PORT\s*=\s*(\d+)".to_string(), "Rails port".to_string()),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec![
            "config/puma.rb",
            "config/application.rb",
            "config/environment.rb",
        ]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if (trimmed.contains("port") || trimmed.contains("bind")) && !trimmed.starts_with('#') {
                if let Some(port) = extract_ruby_port(trimmed) {
                    config.port = Some(port);
                }
            }

            if trimmed.contains("ENV[") || trimmed.contains("ENV.fetch(") {
                extract_ruby_env_vars(trimmed, &mut config.env_vars);
            }
        }

        if config.port.is_some() || !config.env_vars.is_empty() {
            Some(config)
        } else {
            None
        }
    }
}

fn extract_ruby_port(line: &str) -> Option<u16> {
    let num_str: String = line.chars().filter(|c| c.is_numeric()).collect();

    if !num_str.is_empty() {
        num_str.parse::<u16>().ok()
    } else {
        None
    }
}

fn extract_ruby_env_vars(line: &str, env_vars: &mut Vec<String>) {
    let patterns = ["ENV[", "ENV.fetch("];

    for pattern in &patterns {
        if let Some(start) = line.find(pattern) {
            let rest = &line[start + pattern.len()..];

            if let Some(quote_start) = rest.find(['"', '\'']) {
                let quote_char = rest.chars().nth(quote_start).unwrap();
                let after_quote = &rest[quote_start + 1..];

                if let Some(quote_end) = after_quote.find(quote_char) {
                    let var_name = &after_quote[..quote_end];
                    if !var_name.is_empty() && !env_vars.contains(&var_name.to_string()) {
                        env_vars.push(var_name.to_string());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_rails_compatibility() {
        let framework = RailsFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "Ruby"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "bundler"));
    }

    #[test]
    fn test_rails_dependency_detection() {
        let framework = RailsFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "rails".to_string(),
            version: Some("7.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_rails_health_endpoints() {
        let framework = RailsFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/up"));
    }

    #[test]
    fn test_rails_default_ports() {
        let framework = RailsFramework;
        assert_eq!(framework.default_ports(), &[3000]);
    }

    #[test]
    fn test_rails_parse_puma_config() {
        let framework = RailsFramework;
        let content = r#"
# Puma configuration
port ENV.fetch('PORT', 3001)
environment ENV['RAILS_ENV']
bind "tcp://0.0.0.0:#{ENV.fetch('PORT', 3001)}"
"#;

        let config = framework
            .parse_config(Path::new("config/puma.rb"), content)
            .unwrap();

        assert_eq!(config.port, Some(3001));
        assert!(config.env_vars.contains(&"PORT".to_string()));
        assert!(config.env_vars.contains(&"RAILS_ENV".to_string()));
    }

    #[test]
    fn test_rails_config_files() {
        let framework = RailsFramework;
        let files = framework.config_files();

        assert!(files.iter().any(|s| *s == "config/puma.rb"));
        assert!(files.iter().any(|s| *s == "config/application.rb"));
    }
}

//! Phoenix framework for Elixir

use super::*;

pub struct PhoenixFramework;

impl Framework for PhoenixFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Phoenix
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Elixir".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["mix".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"phoenix".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![4000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/health".to_string(), "/api/health".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"PHX_HOST\s*=\s*(\S+)".to_string(), "Phoenix host".to_string()),
            (r"PORT\s*=\s*(\d+)".to_string(), "Phoenix port".to_string()),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec!["config/runtime.exs", "config/prod.exs", "config/config.exs"]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("port:") && !trimmed.starts_with('#') {
                if let Some(port) = extract_elixir_port(trimmed) {
                    config.port = Some(port);
                }
            }

            if trimmed.contains("System.get_env(") || trimmed.contains("System.fetch_env!(") {
                extract_elixir_env_vars(trimmed, &mut config.env_vars);
            }
        }

        if config.port.is_some() || !config.env_vars.is_empty() {
            Some(config)
        } else {
            None
        }
    }
}

fn extract_elixir_port(line: &str) -> Option<u16> {
    if let Some(port_pos) = line.find("port:") {
        let rest = &line[port_pos + 5..];
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

fn extract_elixir_env_vars(line: &str, env_vars: &mut Vec<String>) {
    let patterns = ["System.get_env(", "System.fetch_env!("];

    for pattern in &patterns {
        if let Some(start) = line.find(pattern) {
            let rest = &line[start + pattern.len()..];

            if let Some(quote_start) = rest.find('"') {
                let after_quote = &rest[quote_start + 1..];

                if let Some(quote_end) = after_quote.find('"') {
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
    fn test_phoenix_compatibility() {
        let framework = PhoenixFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "Elixir"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "mix"));
    }

    #[test]
    fn test_phoenix_dependency_detection() {
        let framework = PhoenixFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "phoenix".to_string(),
            version: Some("1.7.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_phoenix_default_ports() {
        let framework = PhoenixFramework;
        assert_eq!(framework.default_ports(), vec![4000]);
    }

    #[test]
    fn test_phoenix_parse_config() {
        let framework = PhoenixFramework;
        let content = r#"
import Config

config :my_app, MyAppWeb.Endpoint,
  http: [port: 4001],
  url: [host: System.get_env("PHX_HOST")],
  secret_key_base: System.fetch_env!("SECRET_KEY_BASE")
"#;

        let config = framework
            .parse_config(Path::new("config/runtime.exs"), content)
            .unwrap();

        assert_eq!(config.port, Some(4001));
        assert!(config.env_vars.contains(&"PHX_HOST".to_string()));
        assert!(config.env_vars.contains(&"SECRET_KEY_BASE".to_string()));
    }

    #[test]
    fn test_phoenix_config_files() {
        let framework = PhoenixFramework;
        let files = framework.config_files();

        assert!(files.iter().any(|s| *s == "config/runtime.exs"));
        assert!(files.iter().any(|s| *s == "config/prod.exs"));
    }
}

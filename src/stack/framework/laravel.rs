//! Laravel framework for PHP

use super::*;

pub struct LaravelFramework;

impl Framework for LaravelFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Laravel
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["PHP".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["composer".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"laravel/framework".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![8000]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/api/health".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"APP_ENV\s*=\s*(\w+)".to_string(), "Laravel environment".to_string()),
            (r"APP_PORT\s*=\s*(\d+)".to_string(), "Laravel port".to_string()),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec![
            "config/app.php",
            "config/database.php",
            "config/services.php",
        ]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("env(") {
                extract_laravel_env(trimmed, &mut config.env_vars);
            }

            if trimmed.contains("'port'") && trimmed.contains("env(") {
                if let Some(port) = extract_port_from_env(trimmed) {
                    config.port = Some(port);
                }
            }
        }

        if config.port.is_some() || !config.env_vars.is_empty() {
            Some(config)
        } else {
            None
        }
    }
}

fn extract_laravel_env(line: &str, env_vars: &mut Vec<String>) {
    let mut pos = 0;
    while let Some(start) = line[pos..].find("env(") {
        let abs_start = pos + start + 4;
        let rest = &line[abs_start..];

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

        pos = abs_start + 1;
    }
}

fn extract_port_from_env(line: &str) -> Option<u16> {
    if let Some(start) = line.find("env(") {
        let rest = &line[start..];
        if let Some(comma) = rest.find(',') {
            let after_comma = &rest[comma + 1..];
            let num_str: String = after_comma
                .chars()
                .skip_while(|c| c.is_whitespace())
                .take_while(|c| c.is_numeric())
                .collect();

            if !num_str.is_empty() {
                return num_str.parse::<u16>().ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_laravel_compatibility() {
        let framework = LaravelFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "PHP"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "composer"));
    }

    #[test]
    fn test_laravel_dependency_detection() {
        let framework = LaravelFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "laravel/framework".to_string(),
            version: Some("10.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_laravel_default_ports() {
        let framework = LaravelFramework;
        assert_eq!(framework.default_ports(), vec![8000]);
    }

    #[test]
    fn test_laravel_parse_config() {
        let framework = LaravelFramework;
        let content = r#"
<?php
return [
    'name' => env('APP_NAME', 'Laravel'),
    'env' => env('APP_ENV', 'production'),
    'url' => env('APP_URL', 'http://localhost'),
    'port' => env('APP_PORT', 8080),
    'timezone' => env('APP_TIMEZONE', 'UTC'),
];
"#;

        let config = framework
            .parse_config(Path::new("config/app.php"), content)
            .unwrap();

        assert_eq!(config.port, Some(8080));
        assert!(config.env_vars.contains(&"APP_NAME".to_string()));
        assert!(config.env_vars.contains(&"APP_ENV".to_string()));
        assert!(config.env_vars.contains(&"APP_URL".to_string()));
        assert!(config.env_vars.contains(&"APP_PORT".to_string()));
    }

    #[test]
    fn test_laravel_config_files() {
        let framework = LaravelFramework;
        let files = framework.config_files();

        assert!(files.iter().any(|s| *s == "config/app.php"));
        assert!(files.iter().any(|s| *s == "config/database.php"));
    }
}

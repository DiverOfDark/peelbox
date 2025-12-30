//! ASP.NET Core framework for .NET

use super::*;

pub struct AspNetFramework;

impl Framework for AspNetFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::AspNetCore
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["C#".to_string(), "F#".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["dotnet".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"Microsoft\.AspNetCore\..*".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"Microsoft\.Extensions\..*".to_string(),
                confidence: 0.85,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![5000, 5001]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string(), "/ready".to_string()]
    }

    fn runtime_env_vars(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("ASPNETCORE_URLS".to_string(), "http://+:8080".to_string());
        env
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"ASPNETCORE_ENVIRONMENT\s*=\s*(\w+)".to_string(), "ASP.NET environment".to_string()),
            (r"ASPNETCORE_URLS\s*=\s*([^\s]+)".to_string(), "ASP.NET URLs".to_string()),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec![
            "appsettings.json",
            "appsettings.Development.json",
            "appsettings.Production.json",
        ]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(kestrel) = json.get("Kestrel") {
                if let Some(endpoints) = kestrel.get("Endpoints") {
                    if config.port.is_none() {
                        if let Some(http) = endpoints.get("Http") {
                            if let Some(url) = http.get("Url").and_then(|v| v.as_str()) {
                                if let Some(port) = extract_port_from_url(url) {
                                    config.port = Some(port);
                                }
                            }
                        }
                    }
                    if config.port.is_none() {
                        if let Some(https) = endpoints.get("Https") {
                            if let Some(url) = https.get("Url").and_then(|v| v.as_str()) {
                                if let Some(port) = extract_port_from_url(url) {
                                    config.port = Some(port);
                                }
                            }
                        }
                    }
                }
            }

            extract_json_env_vars(&json, &mut config.env_vars);
        }

        if config.port.is_some() || !config.env_vars.is_empty() {
            Some(config)
        } else {
            None
        }
    }
}

fn extract_port_from_url(url: &str) -> Option<u16> {
    if let Some(colon_pos) = url.rfind(':') {
        let port_str = &url[colon_pos + 1..];
        port_str.parse::<u16>().ok()
    } else {
        None
    }
}

fn extract_json_env_vars(value: &serde_json::Value, env_vars: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with('$') {
                let var_name = s.trim_start_matches('$');
                if !var_name.is_empty() && !env_vars.contains(&var_name.to_string()) {
                    env_vars.push(var_name.to_string());
                }
            }
        }
        serde_json::Value::Object(obj) => {
            for val in obj.values() {
                extract_json_env_vars(val, env_vars);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr {
                extract_json_env_vars(val, env_vars);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_aspnet_compatibility() {
        let framework = AspNetFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "C#"));
        assert!(framework.compatible_languages().iter().any(|s| s == "F#"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "dotnet"));
    }

    #[test]
    fn test_aspnet_dependency_detection() {
        let framework = AspNetFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "Microsoft.AspNetCore.Mvc".to_string(),
            version: Some("7.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_aspnet_health_endpoints() {
        let framework = AspNetFramework;
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/ready"));
    }

    #[test]
    fn test_aspnet_default_ports() {
        let framework = AspNetFramework;
        assert_eq!(framework.default_ports(), vec![5000, 5001]);
    }

    #[test]
    fn test_aspnet_parse_appsettings() {
        let framework = AspNetFramework;
        let content = r#"{
  "Kestrel": {
    "Endpoints": {
      "Http": {
        "Url": "http://localhost:5050"
      },
      "Https": {
        "Url": "https://localhost:5051"
      }
    }
  },
  "ConnectionStrings": {
    "DefaultConnection": "$DATABASE_URL"
  },
  "ApiKey": "$API_KEY"
}"#;

        let config = framework
            .parse_config(Path::new("appsettings.json"), content)
            .unwrap();

        assert_eq!(config.port, Some(5050));
        assert!(config.env_vars.contains(&"DATABASE_URL".to_string()));
        assert!(config.env_vars.contains(&"API_KEY".to_string()));
    }

    #[test]
    fn test_aspnet_config_files() {
        let framework = AspNetFramework;
        let files = framework.config_files();

        assert!(files.iter().any(|s| *s == "appsettings.json"));
        assert!(files.iter().any(|s| *s == "appsettings.Development.json"));
    }
}

//! Spring Boot framework for Java/Kotlin

use super::*;

pub struct SpringBootFramework;

impl Framework for SpringBootFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::SpringBoot
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Java".to_string(), "Kotlin".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["maven".to_string(), "gradle".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "org.springframework.boot:spring-boot-starter-web".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "org.springframework.boot:spring-boot-starter".to_string(),
                confidence: 0.9,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"org\.springframework\.boot:spring-boot-starter-.*".to_string(),
                confidence: 0.85,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![8080]
    }

    fn health_endpoints(&self, files: &[std::path::PathBuf]) -> Vec<String> {
        // Check if actuator dependency exists (pom.xml or build.gradle containing actuator)
        let has_actuator = files.iter().any(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|name| {
                    name == "pom.xml" || name.ends_with(".gradle") || name.ends_with(".gradle.kts")
                })
                .unwrap_or(false)
        });

        if has_actuator {
            vec![
                "/actuator/health".to_string(),
                "/actuator/health/liveness".to_string(),
                "/actuator/health/readiness".to_string(),
            ]
        } else {
            vec!["/health".to_string()]
        }
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"SERVER_PORT\s*=\s*(\d+)".to_string(),
                "Spring Boot server.port".to_string(),
            ),
            (
                r"SPRING_PROFILES_ACTIVE\s*=\s*(\w+)".to_string(),
                "Spring profiles".to_string(),
            ),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec![
            "application.properties",
            "application.yml",
            "application.yaml",
            "src/main/resources/application.properties",
            "src/main/resources/application.yml",
            "src/main/resources/application.yaml",
        ]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        if content.contains('=') && !content.trim_start().starts_with('#') {
            parse_properties(content, &mut config);
        } else if content.contains(':') {
            parse_yaml(content, &mut config);
        }

        if config.port.is_some() || !config.env_vars.is_empty() || config.health_endpoint.is_some()
        {
            Some(config)
        } else {
            None
        }
    }

    fn customize_build_template(&self, mut template: BuildTemplate) -> BuildTemplate {
        if template.runtime_copy.is_empty()
            || !template
                .runtime_copy
                .iter()
                .any(|(from, _)| from.contains(".jar"))
        {
            template
                .runtime_copy
                .push(("target/*.jar".to_string(), "/app/".to_string()));
        }
        template
    }
}

fn parse_properties(content: &str, config: &mut FrameworkConfig) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == "server.port" {
                if let Ok(port) = value.parse::<u16>() {
                    config.port = Some(port);
                }
            } else if key == "management.endpoints.web.base-path" {
                let health = format!("{}/health", value);
                config.health_endpoint = Some(health);
            }

            if value.contains("${") && value.contains('}') {
                extract_env_vars_from_value(value, &mut config.env_vars);
            }
        }
    }
}

fn parse_yaml(content: &str, config: &mut FrameworkConfig) {
    let mut in_server_section = false;
    let mut in_management_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("server:") {
            in_server_section = true;
            in_management_section = false;
        } else if trimmed.starts_with("management:") {
            in_management_section = true;
            in_server_section = false;
        } else if !trimmed.starts_with(' ') && trimmed.ends_with(':') {
            in_server_section = false;
            in_management_section = false;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            if in_server_section && key == "port" {
                if let Ok(port) = value.parse::<u16>() {
                    config.port = Some(port);
                }
            }

            if in_management_section && trimmed.contains("base-path") {
                let health = format!("{}/health", value);
                config.health_endpoint = Some(health);
            }

            if value.contains("${") && value.contains('}') {
                extract_env_vars_from_value(value, &mut config.env_vars);
            }
        }
    }
}

fn extract_env_vars_from_value(value: &str, env_vars: &mut Vec<String>) {
    let mut chars = value.chars().peekable();
    let mut var_name = String::new();
    let mut in_var = false;

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            in_var = true;
            var_name.clear();
        } else if in_var && (ch == '}' || ch == ':') {
            if !var_name.is_empty() && !env_vars.contains(&var_name) {
                env_vars.push(var_name.clone());
            }
            if ch == ':' {
                for ch in chars.by_ref() {
                    if ch == '}' {
                        break;
                    }
                }
            }
            in_var = false;
        } else if in_var {
            var_name.push(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_spring_boot_compatibility() {
        let framework = SpringBootFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "Java"));
        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "Kotlin"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "maven"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "gradle"));
    }

    #[test]
    fn test_spring_boot_dependency_detection() {
        let framework = SpringBootFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "org.springframework.boot:spring-boot-starter-web".to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_spring_boot_health_endpoints() {
        let framework = SpringBootFramework;
        let files = vec![std::path::PathBuf::from("pom.xml")];
        let endpoints = framework.health_endpoints(&files);

        assert!(endpoints.iter().any(|s| s == "/actuator/health"));
        assert!(endpoints.iter().any(|s| s == "/actuator/health/liveness"));
    }

    #[test]
    fn test_spring_boot_default_ports() {
        let framework = SpringBootFramework;
        assert_eq!(framework.default_ports(), vec![8080]);
    }

    #[test]
    fn test_spring_boot_parse_properties() {
        let framework = SpringBootFramework;
        let content = r#"
server.port=9090
management.endpoints.web.base-path=/management
spring.datasource.url=${DATABASE_URL}
spring.application.name=${APP_NAME:myapp}
"#;

        let config = framework
            .parse_config(Path::new("application.properties"), content)
            .unwrap();

        assert_eq!(config.port, Some(9090));
        assert!(config.env_vars.contains(&"DATABASE_URL".to_string()));
        assert!(config.env_vars.contains(&"APP_NAME".to_string()));
        assert_eq!(
            config.health_endpoint,
            Some("/management/health".to_string())
        );
    }

    #[test]
    fn test_spring_boot_parse_yaml() {
        let framework = SpringBootFramework;
        let content = r#"
server:
  port: 8081

management:
  endpoints:
    web:
      base-path: /actuator

spring:
  datasource:
    url: ${DB_URL}
"#;

        let config = framework
            .parse_config(Path::new("application.yml"), content)
            .unwrap();

        assert_eq!(config.port, Some(8081));
        assert!(config.env_vars.contains(&"DB_URL".to_string()));
    }

    #[test]
    fn test_spring_boot_config_files() {
        let framework = SpringBootFramework;
        let files = framework.config_files();

        assert!(files.contains(&"application.properties"));
        assert!(files.contains(&"application.yml"));
        assert!(files.contains(&"application.yaml"));
    }
}

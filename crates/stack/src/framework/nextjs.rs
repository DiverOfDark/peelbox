//! Next.js framework for JavaScript/TypeScript

use super::*;

pub struct NextJsFramework;

impl Framework for NextJsFramework {
    fn id(&self) -> crate::FrameworkId {
        crate::FrameworkId::NextJs
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["JavaScript".to_string(), "TypeScript".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec![
            "npm".to_string(),
            "yarn".to_string(),
            "pnpm".to_string(),
            "bun".to_string(),
        ]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::NpmPackage,
            pattern: "next".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![3000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/api/health".to_string(), "/health".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"PORT\s*=\s*(\d+)".to_string(), "Next.js port".to_string()),
            (
                r"NODE_ENV\s*=\s*(\w+)".to_string(),
                "Node environment".to_string(),
            ),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec!["next.config.js", "next.config.ts", "next.config.mjs"]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("port") && trimmed.contains(':') {
                if let Some(port) = extract_js_port(trimmed) {
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

    fn customize_build_template(&self, mut template: BuildTemplate) -> BuildTemplate {
        if !template
            .runtime_copy
            .iter()
            .any(|(from, _)| from.contains(".next"))
        {
            template
                .runtime_copy
                .push((".next/".to_string(), "/app/.next".to_string()));
            template
                .runtime_copy
                .push(("public/".to_string(), "/app/public".to_string()));
        }
        template
    }
}

fn extract_js_port(line: &str) -> Option<u16> {
    let num_str: String = line
        .chars()
        .skip_while(|c| !c.is_numeric())
        .take_while(|c| c.is_numeric())
        .collect();

    if !num_str.is_empty() {
        num_str.parse::<u16>().ok()
    } else {
        None
    }
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
    use crate::language::Dependency;

    #[test]
    fn test_nextjs_compatibility() {
        let framework = NextJsFramework;

        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "JavaScript"));
        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "TypeScript"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "npm"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "yarn"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "pnpm"));
    }

    #[test]
    fn test_nextjs_dependency_detection() {
        let framework = NextJsFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "next".to_string(),
            version: Some("14.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_nextjs_health_endpoints() {
        let framework = NextJsFramework;
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/api/health"));
        assert!(endpoints.iter().any(|s| s == "/health"));
    }

    #[test]
    fn test_nextjs_default_ports() {
        let framework = NextJsFramework;
        assert_eq!(framework.default_ports(), vec![3000]);
    }

    #[test]
    fn test_nextjs_parse_config() {
        let framework = NextJsFramework;
        let content = r#"
const nextConfig = {
  env: {
    API_URL: process.env.API_URL,
    PUBLIC_KEY: process.env.PUBLIC_KEY,
  },
  serverRuntimeConfig: {
    port: 3001,
  },
}
module.exports = nextConfig
"#;

        let config = framework
            .parse_config(Path::new("next.config.js"), content)
            .unwrap();

        assert_eq!(config.port, Some(3001));
        assert!(config.env_vars.contains(&"API_URL".to_string()));
        assert!(config.env_vars.contains(&"PUBLIC_KEY".to_string()));
    }

    #[test]
    fn test_nextjs_config_files() {
        let framework = NextJsFramework;
        let files = framework.config_files();

        assert!(files.contains(&"next.config.js"));
        assert!(files.contains(&"next.config.ts"));
    }
}

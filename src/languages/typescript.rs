//! TypeScript language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

pub struct TypeScriptLanguage;

impl LanguageDefinition for TypeScriptLanguage {
    fn name(&self) -> &str {
        "TypeScript"
    }

    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "mts", "cts"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "tsconfig.json",
                build_system: "npm",
                priority: 8,
            },
            ManifestPattern {
                filename: "package.json",
                build_system: "npm",
                priority: 5,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        match manifest_name {
            "tsconfig.json" => Some(DetectionResult {
                build_system: "npm".to_string(),
                confidence: 0.95,
            }),
            "package.json" => {
                if let Some(content) = manifest_content {
                    if content.contains("\"typescript\"")
                        || content.contains("\"@types/")
                        || content.contains("\"ts-node\"")
                    {
                        return Some(DetectionResult {
                            build_system: "npm".to_string(),
                            confidence: 0.9,
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        match build_system {
            "npm" | "yarn" | "pnpm" | "bun" => Some(BuildTemplate {
                build_image: "node:20".to_string(),
                runtime_image: "node:20-slim".to_string(),
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec!["npm ci".to_string(), "npm run build".to_string()],
                cache_paths: vec!["node_modules/".to_string(), ".npm/".to_string()],
                artifacts: vec!["dist/".to_string(), "build/".to_string()],
                common_ports: vec![3000, 8080],
            }),
            _ => None,
        }
    }

    fn build_systems(&self) -> &[&str] {
        &["npm", "yarn", "pnpm", "bun"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = TypeScriptLanguage;
        assert_eq!(lang.name(), "TypeScript");
    }

    #[test]
    fn test_extensions() {
        let lang = TypeScriptLanguage;
        assert!(lang.extensions().contains(&"ts"));
        assert!(lang.extensions().contains(&"tsx"));
    }

    #[test]
    fn test_detect_tsconfig() {
        let lang = TypeScriptLanguage;
        let result = lang.detect("tsconfig.json", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "npm");
        assert_eq!(r.confidence, 0.95);
    }

    #[test]
    fn test_detect_package_json_with_typescript() {
        let lang = TypeScriptLanguage;
        let content = r#"{"devDependencies": {"typescript": "^5.0.0"}}"#;
        let result = lang.detect("package.json", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 0.9);
    }

    #[test]
    fn test_detect_package_json_with_types() {
        let lang = TypeScriptLanguage;
        let content = r#"{"devDependencies": {"@types/node": "^20.0.0"}}"#;
        let result = lang.detect("package.json", Some(content));
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_package_json_no_typescript() {
        let lang = TypeScriptLanguage;
        let content = r#"{"dependencies": {"express": "^4.0.0"}}"#;
        let result = lang.detect("package.json", Some(content));
        assert!(result.is_none());
    }

    #[test]
    fn test_build_template() {
        let lang = TypeScriptLanguage;
        let template = lang.build_template("npm");
        assert!(template.is_some());
        let t = template.unwrap();
        assert_eq!(t.build_image, "node:20");
    }
}

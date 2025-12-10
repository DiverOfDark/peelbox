//! JavaScript language definition (npm, yarn, pnpm, bun)

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

pub struct JavaScriptLanguage;

impl LanguageDefinition for JavaScriptLanguage {
    fn name(&self) -> &str {
        "JavaScript"
    }

    fn extensions(&self) -> &[&str] {
        &["js", "mjs", "cjs", "jsx"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "package.json",
                build_system: "npm",
                priority: 10,
            },
            ManifestPattern {
                filename: "yarn.lock",
                build_system: "yarn",
                priority: 15,
            },
            ManifestPattern {
                filename: "pnpm-lock.yaml",
                build_system: "pnpm",
                priority: 15,
            },
            ManifestPattern {
                filename: "bun.lockb",
                build_system: "bun",
                priority: 15,
            },
            ManifestPattern {
                filename: "package-lock.json",
                build_system: "npm",
                priority: 12,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        match manifest_name {
            "bun.lockb" => Some(DetectionResult {
                build_system: "bun".to_string(),
                confidence: 1.0,
            }),
            "pnpm-lock.yaml" => Some(DetectionResult {
                build_system: "pnpm".to_string(),
                confidence: 1.0,
            }),
            "yarn.lock" => Some(DetectionResult {
                build_system: "yarn".to_string(),
                confidence: 1.0,
            }),
            "package-lock.json" => Some(DetectionResult {
                build_system: "npm".to_string(),
                confidence: 1.0,
            }),
            "package.json" => {
                let mut confidence = 0.8;
                let mut build_system = "npm".to_string();

                if let Some(content) = manifest_content {
                    if content.contains("\"name\"") && content.contains("\"version\"") {
                        confidence = 0.9;
                    }
                    if content.contains("\"packageManager\": \"pnpm") {
                        build_system = "pnpm".to_string();
                        confidence = 0.95;
                    } else if content.contains("\"packageManager\": \"yarn") {
                        build_system = "yarn".to_string();
                        confidence = 0.95;
                    } else if content.contains("\"packageManager\": \"bun") {
                        build_system = "bun".to_string();
                        confidence = 0.95;
                    }
                }

                Some(DetectionResult {
                    build_system,
                    confidence,
                })
            }
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        match build_system {
            "npm" => Some(BuildTemplate {
                build_image: "node:20".to_string(),
                runtime_image: "node:20-slim".to_string(),
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec!["npm ci".to_string(), "npm run build".to_string()],
                cache_paths: vec!["node_modules/".to_string(), ".npm/".to_string()],
                artifacts: vec!["dist/".to_string(), "build/".to_string()],
                common_ports: vec![3000, 8080],
            }),
            "yarn" => Some(BuildTemplate {
                build_image: "node:20".to_string(),
                runtime_image: "node:20-slim".to_string(),
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec![
                    "yarn install --frozen-lockfile".to_string(),
                    "yarn build".to_string(),
                ],
                cache_paths: vec!["node_modules/".to_string(), ".yarn/cache/".to_string()],
                artifacts: vec!["dist/".to_string(), "build/".to_string()],
                common_ports: vec![3000, 8080],
            }),
            "pnpm" => Some(BuildTemplate {
                build_image: "node:20".to_string(),
                runtime_image: "node:20-slim".to_string(),
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec![
                    "corepack enable".to_string(),
                    "pnpm install --frozen-lockfile".to_string(),
                    "pnpm build".to_string(),
                ],
                cache_paths: vec!["node_modules/".to_string(), ".pnpm-store/".to_string()],
                artifacts: vec!["dist/".to_string(), "build/".to_string()],
                common_ports: vec![3000, 8080],
            }),
            "bun" => Some(BuildTemplate {
                build_image: "oven/bun:1".to_string(),
                runtime_image: "oven/bun:1-slim".to_string(),
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec!["bun install".to_string(), "bun run build".to_string()],
                cache_paths: vec!["node_modules/".to_string(), ".bun/".to_string()],
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
        let lang = JavaScriptLanguage;
        assert_eq!(lang.name(), "JavaScript");
    }

    #[test]
    fn test_extensions() {
        let lang = JavaScriptLanguage;
        assert!(lang.extensions().contains(&"js"));
        assert!(lang.extensions().contains(&"jsx"));
    }

    #[test]
    fn test_detect_package_json() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("package.json", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "npm");
    }

    #[test]
    fn test_detect_yarn_lock() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("yarn.lock", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "yarn");
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_pnpm_lock() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("pnpm-lock.yaml", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "pnpm");
    }

    #[test]
    fn test_detect_bun_lock() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("bun.lockb", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "bun");
    }

    #[test]
    fn test_detect_packagemanager_field() {
        let lang = JavaScriptLanguage;
        let content = r#"{"name": "test", "version": "1.0.0", "packageManager": "pnpm@8.0.0"}"#;
        let result = lang.detect("package.json", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "pnpm");
    }

    #[test]
    fn test_build_template_npm() {
        let lang = JavaScriptLanguage;
        let template = lang.build_template("npm");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("npm")));
    }

    #[test]
    fn test_build_template_yarn() {
        let lang = JavaScriptLanguage;
        let template = lang.build_template("yarn");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("yarn")));
    }

    #[test]
    fn test_build_template_pnpm() {
        let lang = JavaScriptLanguage;
        let template = lang.build_template("pnpm");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("pnpm")));
    }

    #[test]
    fn test_build_template_bun() {
        let lang = JavaScriptLanguage;
        let template = lang.build_template("bun");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("bun"));
    }

    #[test]
    fn test_build_systems() {
        let lang = JavaScriptLanguage;
        let systems = lang.build_systems();
        assert!(systems.contains(&"npm"));
        assert!(systems.contains(&"yarn"));
        assert!(systems.contains(&"pnpm"));
        assert!(systems.contains(&"bun"));
    }
}

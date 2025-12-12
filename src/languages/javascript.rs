//! JavaScript/TypeScript language definition (npm, yarn, pnpm, bun)

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
use regex::Regex;

pub struct JavaScriptLanguage;

impl LanguageDefinition for JavaScriptLanguage {
    fn name(&self) -> &str {
        "JavaScript"
    }

    fn extensions(&self) -> &[&str] {
        &["js", "mjs", "cjs", "jsx", "ts", "tsx", "mts", "cts"]
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
            ManifestPattern {
                filename: "tsconfig.json",
                build_system: "npm",
                priority: 8,
            },
            ManifestPattern {
                filename: ".nvmrc",
                build_system: "npm",
                priority: 3,
            },
            ManifestPattern {
                filename: ".node-version",
                build_system: "npm",
                priority: 3,
            },
        ]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
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
            "tsconfig.json" => Some(DetectionResult {
                build_system: "npm".to_string(),
                confidence: 0.9,
            }),
            ".nvmrc" | ".node-version" => Some(DetectionResult {
                build_system: "npm".to_string(),
                confidence: 0.5,
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

    fn excluded_dirs(&self) -> &[&str] {
        &[
            "node_modules",
            "dist",
            "build",
            "out",
            ".next",
            ".nuxt",
            "coverage",
        ]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[
            "pnpm-workspace.yaml",
            "lerna.json",
            "nx.json",
            "turbo.json",
            "rush.json",
        ]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // .nvmrc or .node-version (just contains version number)
        if !content.contains('{') && !content.contains('<') {
            let trimmed = content.trim();
            // Match "20", "20.0", "v20.0.0", "lts/iron", etc.
            if Regex::new(r"^v?(\d+)").ok()?.is_match(trimmed) {
                if let Some(caps) = Regex::new(r"^v?(\d+)").ok()?.captures(trimmed) {
                    return Some(caps.get(1)?.as_str().to_string());
                }
            }
            // LTS codenames map to major versions
            if trimmed.contains("iron") {
                return Some("20".to_string());
            }
            if trimmed.contains("hydrogen") {
                return Some("18".to_string());
            }
        }

        // package.json engines.node
        if let Some(caps) = Regex::new(r#""engines"\s*:\s*\{[^}]*"node"\s*:\s*"[^\d]*(\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // package.json volta.node
        if let Some(caps) = Regex::new(r#""volta"\s*:\s*\{[^}]*"node"\s*:\s*"(\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        None
    }
}

/// Check if a package.json indicates a TypeScript project
#[allow(dead_code)]
pub fn is_typescript_project(manifest_content: Option<&str>) -> bool {
    if let Some(content) = manifest_content {
        content.contains("\"typescript\"")
            || content.contains("\"@types/")
            || content.contains("\"ts-node\"")
            || content.contains("tsconfig.json")
    } else {
        false
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
        assert!(lang.extensions().contains(&"ts"));
        assert!(lang.extensions().contains(&"tsx"));
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
    fn test_detect_tsconfig() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("tsconfig.json", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "npm");
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

    #[test]
    fn test_excluded_dirs() {
        let lang = JavaScriptLanguage;
        assert!(lang.excluded_dirs().contains(&"node_modules"));
        assert!(lang.excluded_dirs().contains(&".next"));
    }

    #[test]
    fn test_workspace_configs() {
        let lang = JavaScriptLanguage;
        assert!(lang.workspace_configs().contains(&"pnpm-workspace.yaml"));
        assert!(lang.workspace_configs().contains(&"turbo.json"));
    }

    #[test]
    fn test_detect_version_nvmrc() {
        let lang = JavaScriptLanguage;
        assert_eq!(lang.detect_version(Some("20")), Some("20".to_string()));
        assert_eq!(lang.detect_version(Some("v20.0.0")), Some("20".to_string()));
    }

    #[test]
    fn test_detect_version_lts() {
        let lang = JavaScriptLanguage;
        assert_eq!(
            lang.detect_version(Some("lts/iron")),
            Some("20".to_string())
        );
    }

    #[test]
    fn test_detect_version_engines() {
        let lang = JavaScriptLanguage;
        let content = r#"{"engines": {"node": ">=18"}}"#;
        assert_eq!(lang.detect_version(Some(content)), Some("18".to_string()));
    }

    #[test]
    fn test_is_typescript_project() {
        assert!(is_typescript_project(Some(
            r#"{"devDependencies": {"typescript": "^5.0"}}"#
        )));
        assert!(is_typescript_project(Some(
            r#"{"devDependencies": {"@types/node": "^20"}}"#
        )));
        assert!(!is_typescript_project(Some(r#"{"dependencies": {}}"#)));
        assert!(!is_typescript_project(None));
    }
}

//! JavaScript/TypeScript language definition (npm, yarn, pnpm, bun)

use super::{
    BuildTemplate, Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition, ManifestPattern,
};
use regex::Regex;
use std::collections::HashSet;

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

    fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "package.json" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let parsed: serde_json::Value = match serde_json::from_str(manifest_content) {
            Ok(v) => v,
            Err(_) => return DependencyInfo::empty(),
        };

        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        // Extract dependencies and devDependencies
        for dep_type in &["dependencies", "devDependencies", "peerDependencies"] {
            if let Some(deps) = parsed.get(dep_type).and_then(|v| v.as_object()) {
                for (name, version) in deps {
                    if seen.contains(name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let version_str = version.as_str().map(|s| s.to_string());

                    // Check if it's a workspace reference (file:, workspace:, link:)
                    let is_internal = if let Some(v) = version_str.as_deref() {
                        v.starts_with("file:")
                            || v.starts_with("workspace:")
                            || v.starts_with("link:")
                    } else {
                        false
                    };

                    let dep = Dependency {
                        name: name.clone(),
                        version: version_str,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
                    }
                }
            }
        }

        // Check for workspace packages
        if let Some(workspaces) = parsed.get("workspaces") {
            let workspace_patterns: Vec<String> = if let Some(arr) = workspaces.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(obj) = workspaces.as_object() {
                if let Some(packages) = obj.get("packages").and_then(|v| v.as_array()) {
                    packages
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            // Match workspace patterns against internal paths
            for pattern in workspace_patterns {
                for path in all_internal_paths {
                    let path_str = path.display().to_string();
                    if self.matches_workspace_pattern(&path_str, &pattern) {
                        // Extract package name from path (e.g., "packages/foo" -> "foo")
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if !seen.contains(name) {
                                internal_deps.push(Dependency {
                                    name: name.to_string(),
                                    version: Some("workspace:*".to_string()),
                                    is_internal: true,
                                });
                                seen.insert(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r"process\.env\.([A-Z_][A-Z0-9_]*)", "process.env")]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"app\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "Express")]
    }

    fn is_main_file(&self, _fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(filename) = file_path.file_name().and_then(|f| f.to_str()) {
            let entry_files = [
                "index.js",
                "server.js",
                "app.js",
                "main.js",
                "index.ts",
                "server.ts",
                "app.ts",
                "main.ts",
                "index.mjs",
                "index.cjs",
            ];
            entry_files.contains(&filename)
        } else {
            false
        }
    }

    fn default_health_endpoints(&self) -> Vec<(&'static str, &'static str)> {
        vec![("/health", "Express")]
    }

    fn default_env_vars(&self) -> Vec<&'static str> {
        vec![]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"\.listen\s*\(\s*(\d{4,5})", "listen()"),
            (r"port\s*:\s*(\d{4,5})", "port config"),
        ]
    }
}

impl JavaScriptLanguage {
    fn matches_workspace_pattern(&self, path: &str, pattern: &str) -> bool {
        // Simple glob matching for workspace patterns
        // Supports: packages/*, apps/*, libs/*
        let pattern_regex = pattern.replace('*', "[^/]+");
        if let Ok(re) = Regex::new(&format!("^{}$", pattern_regex)) {
            re.is_match(path)
        } else {
            false
        }
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
    fn test_parse_dependencies_simple() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "react": "^18.0.0",
                "express": "^4.18.0"
            },
            "devDependencies": {
                "typescript": "^5.0.0"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 3);
        assert_eq!(deps.internal_deps.len(), 0);
        assert!(deps.external_deps.iter().any(|d| d.name == "react"));
        assert!(deps.external_deps.iter().any(|d| d.name == "express"));
        assert!(deps.external_deps.iter().any(|d| d.name == "typescript"));
    }

    #[test]
    fn test_parse_dependencies_workspace() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "react": "^18.0.0",
                "@myapp/shared": "workspace:*"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.internal_deps[0].name, "@myapp/shared");
        assert!(deps.internal_deps[0].is_internal);
    }

    #[test]
    fn test_parse_dependencies_pnpm_workspace() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "lodash": "^4.17.21",
                "@myapp/core": "workspace:*",
                "@myapp/utils": "workspace:^1.0.0"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 2);
        assert!(deps.internal_deps.iter().any(|d| d.name == "@myapp/core"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "@myapp/utils"));
    }

    #[test]
    fn test_parse_dependencies_file_protocol() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "express": "^4.18.0",
                "@myapp/shared": "file:../shared",
                "@myapp/utils": "file:packages/utils"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 2);
        assert!(deps.internal_deps.iter().all(|d| d.is_internal));
    }

    #[test]
    fn test_parse_dependencies_link_protocol() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "react": "^18.0.0",
                "local-lib": "link:../local-lib"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.internal_deps[0].name, "local-lib");
        assert!(deps.internal_deps[0].is_internal);
    }

    #[test]
    fn test_parse_dependencies_npm_workspaces_array() {
        use std::path::PathBuf;

        let lang = JavaScriptLanguage;
        let content = r#"{
            "workspaces": [
                "packages/*",
                "apps/*"
            ],
            "dependencies": {
                "express": "^4.18.0"
            }
        }"#;

        let internal_paths = vec![
            PathBuf::from("packages/core"),
            PathBuf::from("packages/utils"),
            PathBuf::from("apps/web"),
        ];

        let deps = lang.parse_dependencies(content, &internal_paths);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 3);
        assert!(deps.internal_deps.iter().any(|d| d.name == "core"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "utils"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "web"));
    }

    #[test]
    fn test_parse_dependencies_yarn_workspaces_object() {
        use std::path::PathBuf;

        let lang = JavaScriptLanguage;
        let content = r#"{
            "workspaces": {
                "packages": [
                    "packages/*",
                    "libs/*"
                ]
            },
            "dependencies": {
                "lodash": "^4.17.21"
            }
        }"#;

        let internal_paths = vec![PathBuf::from("packages/api"), PathBuf::from("libs/shared")];

        let deps = lang.parse_dependencies(content, &internal_paths);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 2);
        assert!(deps.internal_deps.iter().any(|d| d.name == "api"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "shared"));
        assert!(deps
            .internal_deps
            .iter()
            .all(|d| d.version == Some("workspace:*".to_string())));
    }

    #[test]
    fn test_parse_dependencies_peer_dependencies() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "react": "^18.0.0"
            },
            "peerDependencies": {
                "react-dom": "^18.0.0"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps.external_deps.iter().any(|d| d.name == "react"));
        assert!(deps.external_deps.iter().any(|d| d.name == "react-dom"));
    }

    #[test]
    fn test_parse_dependencies_deduplication() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "dependencies": {
                "lodash": "^4.17.21"
            },
            "devDependencies": {
                "lodash": "^4.17.21"
            }
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.external_deps[0].name, "lodash");
    }

    #[test]
    fn test_parse_dependencies_invalid_json() {
        let lang = JavaScriptLanguage;
        let content = "not valid json {";
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 0);
        assert_eq!(deps.internal_deps.len(), 0);
        assert_eq!(deps.detected_by, DetectionMethod::NotImplemented);
    }

    #[test]
    fn test_parse_dependencies_empty_package_json() {
        let lang = JavaScriptLanguage;
        let content = r#"{
            "name": "empty-project",
            "version": "1.0.0"
        }"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 0);
        assert_eq!(deps.internal_deps.len(), 0);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
    }
}

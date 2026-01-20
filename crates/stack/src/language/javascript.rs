//! JavaScript/TypeScript language definition (npm, yarn, pnpm, bun)

#[cfg(test)]
use super::DetectionMethod;
use super::{
    parsers::{DependencyParser, JsonDependencyParser},
    DependencyInfo, DetectionResult, LanguageDefinition,
};
use regex::Regex;

pub struct JavaScriptLanguage;

impl LanguageDefinition for JavaScriptLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::JavaScript
    }

    fn extensions(&self) -> Vec<String> {
        vec![
            "js".to_string(),
            "mjs".to_string(),
            "cjs".to_string(),
            "jsx".to_string(),
            "ts".to_string(),
            "tsx".to_string(),
            "mts".to_string(),
            "cts".to_string(),
        ]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "bun.lockb" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Bun,
                confidence: 1.0,
            }),
            "pnpm-lock.yaml" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Pnpm,
                confidence: 1.0,
            }),
            "yarn.lock" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Yarn,
                confidence: 1.0,
            }),
            "package-lock.json" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Npm,
                confidence: 1.0,
            }),
            "tsconfig.json" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Npm,
                confidence: 0.9,
            }),
            ".nvmrc" | ".node-version" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Npm,
                confidence: 0.5,
            }),
            "package.json" => {
                let mut confidence = 0.8;
                let mut build_system = crate::BuildSystemId::Npm;

                if let Some(content) = manifest_content {
                    if content.contains("\"name\"") && content.contains("\"version\"") {
                        confidence = 0.9;
                    }
                    if content.contains("\"packageManager\": \"pnpm") {
                        build_system = crate::BuildSystemId::Pnpm;
                        confidence = 0.95;
                    } else if content.contains("\"packageManager\": \"yarn") {
                        build_system = crate::BuildSystemId::Yarn;
                        confidence = 0.95;
                    } else if content.contains("\"packageManager\": \"bun") {
                        build_system = crate::BuildSystemId::Bun;
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

    fn compatible_build_systems(&self) -> Vec<String> {
        vec![
            "npm".to_string(),
            "yarn".to_string(),
            "pnpm".to_string(),
            "bun".to_string(),
        ]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec![
            "node_modules".to_string(),
            "dist".to_string(),
            "build".to_string(),
            "out".to_string(),
            ".next".to_string(),
            ".nuxt".to_string(),
            "coverage".to_string(),
        ]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![
            "pnpm-workspace.yaml".to_string(),
            "lerna.json".to_string(),
            "nx.json".to_string(),
            "turbo.json".to_string(),
            "rush.json".to_string(),
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
        JsonDependencyParser {
            dependencies_keys: &["dependencies", "devDependencies", "peerDependencies"],
            workspace_key: Some("workspaces"),
        }
        .parse(manifest_content, all_internal_paths)
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r"process\.env\.([A-Z_][A-Z0-9_]*)".to_string(),
            "process.env".to_string(),
        )]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"app\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#.to_string(),
            "Express".to_string(),
        )]
    }

    fn is_main_file(
        &self,
        _fs: &dyn peelbox_core::fs::FileSystem,
        file_path: &std::path::Path,
    ) -> bool {
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

    fn default_health_endpoints(&self) -> Vec<(String, String)> {
        vec![("/health".to_string(), "Express".to_string())]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"\.listen\s*\(\s*(\d{4,5})".to_string(),
                "listen()".to_string(),
            ),
            (
                r"port\s*:\s*(\d{4,5})".to_string(),
                "port config".to_string(),
            ),
        ]
    }

    fn runtime_name(&self) -> Option<String> {
        Some("node".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(3000)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        None
    }

    fn parse_entrypoint_from_manifest(&self, manifest_content: &str) -> Option<String> {
        let parsed: serde_json::Value = serde_json::from_str(manifest_content).ok()?;

        if let Some(main) = parsed.get("main").and_then(|v| v.as_str()) {
            return Some(format!("node {}", main));
        }

        if let Some(scripts) = parsed.get("scripts") {
            if let Some(start) = scripts.get("start").and_then(|v| v.as_str()) {
                return Some(start.to_string());
            }
        }

        None
    }

    fn find_entrypoints(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        _repo_root: &std::path::Path,
        project_root: &std::path::Path,
        file_tree: &[std::path::PathBuf],
    ) -> Vec<String> {
        let mut entrypoints = Vec::new();
        for file_path in file_tree {
            if self.is_main_file(fs, &project_root.join(file_path)) {
                entrypoints.push(file_path.to_string_lossy().to_string());
            }
        }
        entrypoints
    }

    fn is_runnable(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        repo_root: &std::path::Path,
        project_root: &std::path::Path,
        file_tree: &[std::path::PathBuf],
        manifest_content: Option<&str>,
    ) -> bool {
        if let Some(content) = manifest_content {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
                if let Some(scripts) = parsed.get("scripts").and_then(|s| s.as_object()) {
                    if scripts.contains_key("start") || scripts.contains_key("dev") {
                        return true;
                    }
                }
            }
        }

        !self
            .find_entrypoints(fs, repo_root, project_root, file_tree)
            .is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = JavaScriptLanguage;
        assert!(lang.extensions().iter().any(|s| s == "js"));
        assert!(lang.extensions().iter().any(|s| s == "jsx"));
        assert!(lang.extensions().iter().any(|s| s == "ts"));
        assert!(lang.extensions().iter().any(|s| s == "tsx"));
    }

    #[test]
    fn test_detect_package_json() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("package.json", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Npm);
    }

    #[test]
    fn test_detect_yarn_lock() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("yarn.lock", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Yarn);
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_pnpm_lock() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("pnpm-lock.yaml", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Pnpm);
    }

    #[test]
    fn test_detect_bun_lock() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("bun.lockb", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Bun);
    }

    #[test]
    fn test_detect_tsconfig() {
        let lang = JavaScriptLanguage;
        let result = lang.detect("tsconfig.json", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Npm);
    }

    #[test]
    fn test_detect_packagemanager_field() {
        let lang = JavaScriptLanguage;
        let content = r#"{"name": "test", "version": "1.0.0", "packageManager": "pnpm@8.0.0"}"#;
        let result = lang.detect("package.json", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Pnpm);
    }

    #[test]
    fn test_compatible_build_systems() {
        let lang = JavaScriptLanguage;
        let systems = lang.compatible_build_systems();
        assert!(systems.iter().any(|s| s == "npm"));
        assert!(systems.iter().any(|s| s == "yarn"));
        assert!(systems.iter().any(|s| s == "pnpm"));
        assert!(systems.iter().any(|s| s == "bun"));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = JavaScriptLanguage;
        assert!(lang.excluded_dirs().iter().any(|s| s == "node_modules"));
        assert!(lang.excluded_dirs().iter().any(|s| s == ".next"));
    }

    #[test]
    fn test_workspace_configs() {
        let lang = JavaScriptLanguage;
        assert!(lang
            .workspace_configs()
            .iter()
            .any(|s| s == "pnpm-workspace.yaml"));
        assert!(lang.workspace_configs().iter().any(|s| s == "turbo.json"));
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

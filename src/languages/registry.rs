use super::{LanguageDefinition, LanguageDetection};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct LanguageRegistry {
    languages: Vec<Arc<dyn LanguageDefinition>>,
    manifest_index: HashMap<String, Vec<(usize, u8)>>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
            manifest_index: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(super::RustLanguage));
        registry.register(Arc::new(super::JavaLanguage));
        registry.register(Arc::new(super::JavaScriptLanguage));
        registry.register(Arc::new(super::PythonLanguage));
        registry.register(Arc::new(super::GoLanguage));
        registry.register(Arc::new(super::DotNetLanguage));
        registry.register(Arc::new(super::RubyLanguage));
        registry.register(Arc::new(super::PhpLanguage));
        registry.register(Arc::new(super::CppLanguage));
        registry.register(Arc::new(super::ElixirLanguage));
        registry
    }

    pub fn all_excluded_dirs(&self) -> Vec<&str> {
        let mut dirs: Vec<&str> = Vec::new();
        for lang in &self.languages {
            for dir in lang.excluded_dirs() {
                if !dirs.contains(dir) {
                    dirs.push(dir);
                }
            }
        }
        for dir in &[".git", ".idea", ".vscode", "vendor"] {
            if !dirs.contains(dir) {
                dirs.push(dir);
            }
        }
        dirs
    }

    pub fn all_workspace_configs(&self) -> Vec<&str> {
        let mut configs: Vec<&str> = Vec::new();
        for lang in &self.languages {
            for config in lang.workspace_configs() {
                if !configs.contains(config) {
                    configs.push(config);
                }
            }
        }
        configs
    }

    pub fn register(&mut self, language: Arc<dyn LanguageDefinition>) {
        let lang_idx = self.languages.len();

        for pattern in language.manifest_files() {
            self.manifest_index
                .entry(pattern.filename.to_string())
                .or_default()
                .push((lang_idx, pattern.priority));
        }

        self.languages.push(language);
    }

    pub fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<LanguageDetection> {
        let candidates = self.manifest_index.get(manifest_name)?;

        let mut best_result: Option<(LanguageDetection, u8)> = None;

        for &(lang_idx, priority) in candidates {
            let language = &self.languages[lang_idx];

            if let Some(result) = language.detect(manifest_name, manifest_content) {
                let detection = LanguageDetection {
                    language: language.name().to_string(),
                    build_system: result.build_system,
                    confidence: result.confidence,
                    manifest_path: manifest_name.to_string(),
                };

                match &best_result {
                    None => best_result = Some((detection, priority)),
                    Some((_, best_priority)) if priority > *best_priority => {
                        best_result = Some((detection, priority))
                    }
                    Some((best, _)) if detection.confidence > best.confidence => {
                        best_result = Some((detection, priority))
                    }
                    _ => {}
                }
            }
        }

        best_result.map(|(detection, _)| detection)
    }

    pub fn detect_all(&self, manifests: &[(String, Option<String>)]) -> Vec<LanguageDetection> {
        let mut detections = Vec::new();

        for (path, content) in manifests {
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);

            if let Some(mut detection) = self.detect(filename, content.as_deref()) {
                detection.manifest_path = path.clone();
                detections.push(detection);
            }
        }

        detections.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        detections
    }

    /// Get a language definition by name
    pub fn get_language(&self, name: &str) -> Option<&dyn LanguageDefinition> {
        self.languages
            .iter()
            .find(|l| l.name().eq_ignore_ascii_case(name))
            .map(|l| l.as_ref())
    }

    /// Get all registered language names
    pub fn language_names(&self) -> Vec<&str> {
        self.languages.iter().map(|l| l.name()).collect()
    }

    /// Check if a filename is a known manifest
    pub fn is_manifest(&self, filename: &str) -> bool {
        self.manifest_index.contains_key(filename)
    }

    /// Check if a manifest is a workspace root (monorepo indicator)
    pub fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if let Some(candidates) = self.manifest_index.get(manifest_name) {
            for &(lang_idx, _) in candidates {
                let language = &self.languages[lang_idx];
                if language.is_workspace_root(manifest_name, manifest_content) {
                    return true;
                }
            }
        }
        false
    }

    /// Parse dependencies from a manifest file
    pub fn parse_dependencies_by_manifest(
        &self,
        manifest_name: &str,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> Option<super::DependencyInfo> {
        if let Some(candidates) = self.manifest_index.get(manifest_name) {
            for &(lang_idx, _) in candidates {
                let language = &self.languages[lang_idx];
                if language
                    .detect(manifest_name, Some(manifest_content))
                    .is_some()
                {
                    return Some(language.parse_dependencies(manifest_content, all_internal_paths));
                }
            }
        }
        None
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::DetectionMethod;

    #[test]
    fn test_registry_creation() {
        let registry = LanguageRegistry::new();
        assert!(registry.language_names().is_empty());
    }

    #[test]
    fn test_registry_with_defaults() {
        let registry = LanguageRegistry::with_defaults();
        assert!(registry.language_names().contains(&"Rust"));
    }

    #[test]
    fn test_detect_rust() {
        let registry = LanguageRegistry::with_defaults();
        let detection = registry.detect("Cargo.toml", None);

        assert!(detection.is_some());
        let d = detection.unwrap();
        assert_eq!(d.language, "Rust");
        assert_eq!(d.build_system, "cargo");
    }

    #[test]
    fn test_is_manifest() {
        let registry = LanguageRegistry::with_defaults();
        assert!(registry.is_manifest("Cargo.toml"));
        assert!(!registry.is_manifest("README.md"));
    }

    #[test]
    fn test_get_language() {
        let registry = LanguageRegistry::with_defaults();
        let rust = registry.get_language("rust");
        assert!(rust.is_some());
        assert_eq!(rust.unwrap().name(), "Rust");

        let unknown = registry.get_language("cobol");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_detect_all() {
        let registry = LanguageRegistry::with_defaults();
        let manifests = vec![
            ("Cargo.toml".to_string(), None),
            ("src/lib.rs".to_string(), None),
        ];

        let detections = registry.detect_all(&manifests);
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].language, "Rust");
    }

    #[test]
    fn test_all_excluded_dirs() {
        let registry = LanguageRegistry::with_defaults();
        let excluded = registry.all_excluded_dirs();

        // Should include common dirs added by registry
        assert!(excluded.contains(&".git"));
        assert!(excluded.contains(&".idea"));
        assert!(excluded.contains(&".vscode"));
        assert!(excluded.contains(&"vendor"));

        // Should include language-specific dirs
        assert!(excluded.contains(&"target"));
        assert!(excluded.contains(&"node_modules"));
        assert!(excluded.contains(&"__pycache__"));

        // Should NOT include regular directories
        assert!(!excluded.contains(&"packages"));
        assert!(!excluded.contains(&"src"));
    }

    #[test]
    fn test_all_workspace_configs() {
        let registry = LanguageRegistry::with_defaults();
        let configs = registry.all_workspace_configs();

        // Should include workspace configs from languages
        assert!(configs.contains(&"pnpm-workspace.yaml"));
        assert!(configs.contains(&"go.work"));
    }

    #[test]
    fn test_parse_dependencies_cargo_toml() {
        let registry = LanguageRegistry::with_defaults();
        let content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
"#;

        let deps = registry.parse_dependencies_by_manifest("Cargo.toml", content, &[]);
        assert!(deps.is_some());

        let deps = deps.unwrap();
        assert_eq!(deps.external_deps.len(), 2);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.iter().any(|d| d.name == "serde"));
        assert!(deps.external_deps.iter().any(|d| d.name == "tokio"));
    }

    #[test]
    fn test_parse_dependencies_package_json() {
        let registry = LanguageRegistry::with_defaults();
        let content = r#"{
            "name": "test",
            "dependencies": {
                "react": "^18.0.0",
                "express": "^4.18.0"
            },
            "devDependencies": {
                "typescript": "^5.0.0"
            }
        }"#;

        let deps = registry.parse_dependencies_by_manifest("package.json", content, &[]);
        assert!(deps.is_some());

        let deps = deps.unwrap();
        assert_eq!(deps.external_deps.len(), 3);
        assert!(deps.external_deps.iter().any(|d| d.name == "react"));
        assert!(deps.external_deps.iter().any(|d| d.name == "express"));
        assert!(deps.external_deps.iter().any(|d| d.name == "typescript"));
    }

    #[test]
    fn test_parse_dependencies_go_mod() {
        let registry = LanguageRegistry::with_defaults();
        let content = r#"
module github.com/user/project

go 1.21

require (
    github.com/gin-gonic/gin v1.9.0
    github.com/lib/pq v1.10.7
)
"#;

        let deps = registry.parse_dependencies_by_manifest("go.mod", content, &[]);
        assert!(deps.is_some());

        let deps = deps.unwrap();
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "github.com/gin-gonic/gin"));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "github.com/lib/pq"));
    }

    #[test]
    fn test_parse_dependencies_pom_xml() {
        let registry = LanguageRegistry::with_defaults();
        let content = r#"
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
            <version>3.2.0</version>
        </dependency>
        <dependency>
            <groupId>org.postgresql</groupId>
            <artifactId>postgresql</artifactId>
            <version>42.7.0</version>
        </dependency>
    </dependencies>
</project>
"#;

        let deps = registry.parse_dependencies_by_manifest("pom.xml", content, &[]);
        assert!(deps.is_some());

        let deps = deps.unwrap();
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "org.springframework.boot:spring-boot-starter-web"));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "org.postgresql:postgresql"));
    }

    #[test]
    fn test_parse_dependencies_pyproject_toml() {
        let registry = LanguageRegistry::with_defaults();
        let content = r#"
[tool.poetry.dependencies]
python = "^3.11"
flask = "^3.0.0"
pytest = "^7.0.0"
"#;

        let deps = registry.parse_dependencies_by_manifest("pyproject.toml", content, &[]);
        assert!(deps.is_some());

        let deps = deps.unwrap();
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps.external_deps.iter().any(|d| d.name == "flask"));
        assert!(deps.external_deps.iter().any(|d| d.name == "pytest"));
    }

    #[test]
    fn test_parse_dependencies_unknown_manifest() {
        let registry = LanguageRegistry::with_defaults();
        let deps = registry.parse_dependencies_by_manifest("unknown.txt", "content", &[]);
        assert!(deps.is_none());
    }

    #[test]
    fn test_parse_dependencies_invalid_content() {
        let registry = LanguageRegistry::with_defaults();
        let deps = registry.parse_dependencies_by_manifest("package.json", "invalid json {", &[]);
        assert!(deps.is_some());

        let deps = deps.unwrap();
        assert_eq!(deps.external_deps.len(), 0);
        assert_eq!(deps.internal_deps.len(), 0);
    }
}

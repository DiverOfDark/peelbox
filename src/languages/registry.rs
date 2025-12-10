//! Language registry for managing language definitions

use super::{LanguageDefinition, LanguageDetection};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of language definitions for build system detection
#[derive(Clone)]
pub struct LanguageRegistry {
    languages: Vec<Arc<dyn LanguageDefinition>>,
    manifest_index: HashMap<String, Vec<(usize, u8)>>,
}

impl LanguageRegistry {
    /// Creates a new empty registry
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
            manifest_index: HashMap::new(),
        }
    }

    /// Creates a registry with all default languages registered
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(super::RustLanguage));
        registry.register(Arc::new(super::JavaLanguage));
        registry.register(Arc::new(super::KotlinLanguage));
        registry.register(Arc::new(super::JavaScriptLanguage));
        registry.register(Arc::new(super::TypeScriptLanguage));
        registry.register(Arc::new(super::PythonLanguage));
        registry.register(Arc::new(super::GoLanguage));
        registry.register(Arc::new(super::DotNetLanguage));
        registry.register(Arc::new(super::RubyLanguage));
        registry.register(Arc::new(super::PhpLanguage));
        registry.register(Arc::new(super::CppLanguage));
        registry.register(Arc::new(super::ElixirLanguage));
        registry
    }

    /// Register a language definition
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

    /// Detect language from a manifest filename and optional content
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

    /// Detect all languages in a repository from a list of manifest files
    pub fn detect_all(
        &self,
        manifests: &[(String, Option<String>)],
    ) -> Vec<LanguageDetection> {
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
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

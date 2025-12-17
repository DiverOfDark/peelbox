//! Build system registry

use super::BuildSystem;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of build systems
#[derive(Clone)]
pub struct BuildSystemRegistry {
    systems: Vec<Arc<dyn BuildSystem>>,
    manifest_index: HashMap<String, Vec<(usize, u8)>>,
}

impl BuildSystemRegistry {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            manifest_index: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(super::CargoBuildSystem));
        registry.register(Arc::new(super::MavenBuildSystem));
        registry.register(Arc::new(super::GradleBuildSystem));
        registry.register(Arc::new(super::NpmBuildSystem));
        registry.register(Arc::new(super::YarnBuildSystem));
        registry.register(Arc::new(super::PnpmBuildSystem));
        registry.register(Arc::new(super::BunBuildSystem));
        registry.register(Arc::new(super::PipBuildSystem));
        registry.register(Arc::new(super::PoetryBuildSystem));
        registry.register(Arc::new(super::PipenvBuildSystem));
        registry.register(Arc::new(super::GoModBuildSystem));
        registry.register(Arc::new(super::DotNetBuildSystem));
        registry.register(Arc::new(super::ComposerBuildSystem));
        registry.register(Arc::new(super::BundlerBuildSystem));
        registry.register(Arc::new(super::CMakeBuildSystem));
        registry.register(Arc::new(super::MixBuildSystem));
        registry
    }

    pub fn register(&mut self, system: Arc<dyn BuildSystem>) {
        let idx = self.systems.len();

        for pattern in system.manifest_patterns() {
            self.manifest_index
                .entry(pattern.filename.to_string())
                .or_default()
                .push((idx, pattern.priority));
        }

        self.systems.push(system);
    }

    /// Detect build system from manifest
    pub fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<&dyn BuildSystem> {
        let candidates = self.manifest_index.get(manifest_name)?;

        for &(idx, _priority) in candidates {
            let system = &self.systems[idx];
            if system.detect(manifest_name, manifest_content) {
                return Some(system.as_ref());
            }
        }

        None
    }

    /// Get build system by name
    pub fn get(&self, name: &str) -> Option<&dyn BuildSystem> {
        self.systems
            .iter()
            .find(|s| s.name().eq_ignore_ascii_case(name))
            .map(|s| s.as_ref())
    }

    /// Get all workspace configuration files
    pub fn all_workspace_configs(&self) -> Vec<&str> {
        let mut configs = std::collections::HashSet::new();
        for system in &self.systems {
            for config in system.workspace_configs() {
                configs.insert(*config);
            }
        }
        configs.into_iter().collect()
    }

    /// Check if manifest is a known build manifest
    pub fn is_manifest(&self, filename: &str) -> bool {
        self.manifest_index.contains_key(filename)
    }
}

impl Default for BuildSystemRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_with_defaults() {
        let registry = BuildSystemRegistry::with_defaults();
        assert_eq!(registry.systems.len(), 16);
    }

    #[test]
    fn test_detect_cargo() {
        let registry = BuildSystemRegistry::with_defaults();
        let result = registry.detect("Cargo.toml", Some("[package]\nname = \"test\""));
        assert!(result.is_some());
        assert_eq!(result.unwrap().name(), "cargo");
    }

    #[test]
    fn test_detect_npm() {
        let registry = BuildSystemRegistry::with_defaults();
        let result = registry.detect("package-lock.json", None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name(), "npm");
    }

    #[test]
    fn test_detect_yarn() {
        let registry = BuildSystemRegistry::with_defaults();
        let result = registry.detect("yarn.lock", None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name(), "yarn");
    }

    #[test]
    fn test_detect_pnpm() {
        let registry = BuildSystemRegistry::with_defaults();
        let result = registry.detect("pnpm-lock.yaml", None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name(), "pnpm");
    }

    #[test]
    fn test_detect_maven() {
        let registry = BuildSystemRegistry::with_defaults();
        let result = registry.detect("pom.xml", Some("<project>"));
        assert!(result.is_some());
        assert_eq!(result.unwrap().name(), "maven");
    }

    #[test]
    fn test_get_by_name() {
        let registry = BuildSystemRegistry::with_defaults();
        assert!(registry.get("cargo").is_some());
        assert!(registry.get("npm").is_some());
        assert!(registry.get("maven").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_is_manifest() {
        let registry = BuildSystemRegistry::with_defaults();
        assert!(registry.is_manifest("Cargo.toml"));
        assert!(registry.is_manifest("package.json"));
        assert!(registry.is_manifest("pom.xml"));
        assert!(!registry.is_manifest("README.md"));
    }
}

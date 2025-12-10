//! Language definitions and registry for build system detection

mod cpp;
mod dotnet;
mod elixir;
mod go;
mod java;
mod javascript;
mod kotlin;
mod php;
mod python;
mod registry;
mod ruby;
mod rust;
mod typescript;

pub use cpp::CppLanguage;
pub use dotnet::DotNetLanguage;
pub use elixir::ElixirLanguage;
pub use go::GoLanguage;
pub use java::JavaLanguage;
pub use javascript::JavaScriptLanguage;
pub use kotlin::KotlinLanguage;
pub use php::PhpLanguage;
pub use python::PythonLanguage;
pub use registry::LanguageRegistry;
pub use ruby::RubyLanguage;
pub use rust::RustLanguage;
pub use typescript::TypeScriptLanguage;

/// Information about a detected language/build system combination
#[derive(Debug, Clone)]
pub struct LanguageDetection {
    pub language: String,
    pub build_system: String,
    pub confidence: f64,
    pub manifest_path: String,
}

/// Build template for container image generation
#[derive(Debug, Clone)]
pub struct BuildTemplate {
    pub build_image: String,
    pub runtime_image: String,
    pub build_packages: Vec<String>,
    pub runtime_packages: Vec<String>,
    pub build_commands: Vec<String>,
    pub cache_paths: Vec<String>,
    pub artifacts: Vec<String>,
    pub common_ports: Vec<u16>,
}

/// Trait defining a programming language's build characteristics
pub trait LanguageDefinition: Send + Sync {
    /// Language name (e.g., "Rust", "JavaScript")
    fn name(&self) -> &str;

    /// File extensions associated with this language
    fn extensions(&self) -> &[&str];

    /// Manifest files that indicate this language's build systems
    fn manifest_files(&self) -> &[ManifestPattern];

    /// Detect if a manifest file belongs to this language and return build system info
    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult>;

    /// Get best practices template for a specific build system
    fn build_template(&self, build_system: &str) -> Option<BuildTemplate>;

    /// Get all supported build systems
    fn build_systems(&self) -> &[&str];
}

/// Pattern for matching manifest files
#[derive(Debug, Clone)]
pub struct ManifestPattern {
    pub filename: &'static str,
    pub build_system: &'static str,
    pub priority: u8,
}

/// Result of language detection from a manifest
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub build_system: String,
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_template_default() {
        let template = BuildTemplate {
            build_image: "rust:1.75".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            build_packages: vec!["pkg-config".to_string()],
            runtime_packages: vec!["ca-certificates".to_string()],
            build_commands: vec!["cargo build --release".to_string()],
            cache_paths: vec!["target/".to_string()],
            artifacts: vec!["target/release/*".to_string()],
            common_ports: vec![8080],
        };

        assert_eq!(template.build_image, "rust:1.75");
        assert!(!template.build_commands.is_empty());
    }
}

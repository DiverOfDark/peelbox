//! Build system definitions
//!
//! Build systems are first-class entities independent of languages.
//! A language can be compatible with multiple build systems (e.g., JavaScript
//! works with npm, yarn, pnpm, and Bun).

use serde::{Deserialize, Serialize};

/// Build template for container image generation
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Manifest pattern for build system detection
#[derive(Debug, Clone)]
pub struct ManifestPattern {
    pub filename: &'static str,
    pub priority: u8,
}

/// Build system trait
pub trait BuildSystem: Send + Sync {
    fn id(&self) -> crate::stack::BuildSystemId;

    /// Manifest file patterns (e.g., "Cargo.toml", "package.json")
    fn manifest_patterns(&self) -> &[ManifestPattern];

    /// Detect if a manifest belongs to this build system
    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool;

    /// Get build template for this build system
    fn build_template(&self) -> BuildTemplate;

    /// Cache directories for this build system
    fn cache_dirs(&self) -> Vec<String>;

    /// Check if manifest indicates workspace/monorepo root
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        let _ = manifest_content;
        false
    }

    /// Workspace configuration files (e.g., "pnpm-workspace.yaml")
    fn workspace_configs(&self) -> &[&str] {
        &[]
    }
}

/// Workspace-aware build system trait
///
/// Build systems that support monorepo/workspace structures (npm, yarn, pnpm, Cargo, Gradle, Maven)
/// can implement this trait to provide workspace parsing capabilities.
pub trait WorkspaceBuildSystem: BuildSystem {
    /// Parse workspace patterns from manifest (e.g., npm/yarn/pnpm workspaces field, Cargo [workspace])
    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>, anyhow::Error>;

    /// Parse package metadata from manifest (name, is_application)
    fn parse_package_metadata(&self, manifest_content: &str) -> Result<(String, bool), anyhow::Error>;

    /// Glob workspace pattern (e.g., "packages/*") to find package directories
    fn glob_workspace_pattern(&self, repo_path: &std::path::Path, pattern: &str) -> Result<Vec<std::path::PathBuf>, anyhow::Error>;
}

pub mod bun;
pub mod bundler;
pub mod cargo;
pub mod cmake;
pub mod composer;
pub mod dotnet;
pub mod go_mod;
pub mod gradle;
pub mod make;
pub mod maven;
pub mod meson;
pub mod mix;
pub mod npm;
pub mod pip;
pub mod pipenv;
pub mod pnpm;
pub mod poetry;
pub mod yarn;

pub use bun::BunBuildSystem;
pub use bundler::BundlerBuildSystem;
pub use cargo::CargoBuildSystem;
pub use cmake::CMakeBuildSystem;
pub use composer::ComposerBuildSystem;
pub use dotnet::DotNetBuildSystem;
pub use go_mod::GoModBuildSystem;
pub use gradle::GradleBuildSystem;
pub use make::MakeBuildSystem;
pub use maven::MavenBuildSystem;
pub use meson::MesonBuildSystem;
pub use mix::MixBuildSystem;
pub use npm::NpmBuildSystem;
pub use pip::PipBuildSystem;
pub use pipenv::PipenvBuildSystem;
pub use pnpm::PnpmBuildSystem;
pub use poetry::PoetryBuildSystem;
pub use yarn::YarnBuildSystem;

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

pub mod cargo;
pub mod maven;
pub mod gradle;
pub mod npm;
pub mod yarn;
pub mod pnpm;
pub mod bun;
pub mod pip;
pub mod poetry;
pub mod pipenv;
pub mod go_mod;
pub mod dotnet;
pub mod composer;
pub mod bundler;
pub mod cmake;
pub mod mix;

pub use cargo::CargoBuildSystem;
pub use maven::MavenBuildSystem;
pub use gradle::GradleBuildSystem;
pub use npm::NpmBuildSystem;
pub use yarn::YarnBuildSystem;
pub use pnpm::PnpmBuildSystem;
pub use bun::BunBuildSystem;
pub use pip::PipBuildSystem;
pub use poetry::PoetryBuildSystem;
pub use pipenv::PipenvBuildSystem;
pub use go_mod::GoModBuildSystem;
pub use dotnet::DotNetBuildSystem;
pub use composer::ComposerBuildSystem;
pub use bundler::BundlerBuildSystem;
pub use cmake::CMakeBuildSystem;
pub use mix::MixBuildSystem;

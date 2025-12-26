//! Build system definitions
//!
//! Build systems are first-class entities independent of languages.
//! A language can be compatible with multiple build systems (e.g., JavaScript
//! works with npm, yarn, pnpm, and Bun).

use crate::fs::FileSystem;
use crate::stack::DetectionStack;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Build template for container image generation (Wolfi-only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTemplate {
    pub build_packages: Vec<String>,
    pub build_commands: Vec<String>,
    pub cache_paths: Vec<String>,
    pub artifacts: Vec<String>,
    pub common_ports: Vec<u16>,
}

/// Manifest pattern for build system detection
#[derive(Debug, Clone)]
pub struct ManifestPattern {
    pub filename: String,
    pub priority: u8,
}

/// Build system trait
pub trait BuildSystem: Send + Sync {
    fn id(&self) -> crate::stack::BuildSystemId;

    /// Manifest file patterns (e.g., "Cargo.toml", "package.json")
    fn manifest_patterns(&self) -> Vec<ManifestPattern>;

    /// Detect all manifests for this build system in the repository
    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>>;

    /// Get build template for this build system
    /// Uses WolfiPackageIndex for dynamic version discovery
    /// service_path allows build systems to read version hint files (.nvmrc, .python-version, etc.)
    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate;

    /// Cache directories for this build system
    fn cache_dirs(&self) -> Vec<String>;

    /// Check if manifest indicates workspace/monorepo root
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        let _ = manifest_content;
        false
    }

    /// Workspace configuration files (e.g., "pnpm-workspace.yaml")
    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }

    /// Parse package metadata from manifest (name, is_application)
    /// Returns (package_name, is_application). Default fallback returns ("app", true).
    fn parse_package_metadata(
        &self,
        manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        let _ = manifest_content;
        Ok(("app".to_string(), true))
    }

    /// Parse workspace patterns from manifest (e.g., npm/yarn/pnpm workspaces field, Cargo [workspace])
    /// Default implementation returns empty Vec (not a workspace build system)
    fn parse_workspace_patterns(
        &self,
        manifest_content: &str,
    ) -> Result<Vec<String>, anyhow::Error> {
        let _ = manifest_content;
        Ok(vec![])
    }

    /// Glob workspace pattern (e.g., "packages/*") to find package directories
    /// Default implementation handles simple directory paths (used by Cargo, Gradle, Maven, DotNet)
    /// Override for more complex globbing (npm/yarn/pnpm use wildcard patterns)
    fn glob_workspace_pattern(
        &self,
        repo_path: &std::path::Path,
        pattern: &str,
    ) -> Result<Vec<std::path::PathBuf>, anyhow::Error> {
        let project_path = repo_path.join(pattern);
        if project_path.exists() && project_path.is_dir() {
            Ok(vec![project_path])
        } else {
            Ok(vec![])
        }
    }
}

/// Helper function for parsing package.json workspaces field (used by npm, yarn, pnpm)
pub(crate) fn parse_package_json_workspaces(
    manifest_content: &str,
) -> Result<Vec<String>, anyhow::Error> {
    let package: serde_json::Value = serde_json::from_str(manifest_content)?;

    if let Some(workspaces) = package["workspaces"].as_array() {
        Ok(workspaces
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect())
    } else {
        Ok(vec![])
    }
}

/// Helper function for globbing package.json workspace patterns (used by npm, yarn, pnpm)
pub(crate) fn glob_package_json_workspace_pattern(
    repo_path: &std::path::Path,
    pattern: &str,
) -> Result<Vec<std::path::PathBuf>, anyhow::Error> {
    let mut results = Vec::new();

    if pattern.ends_with("/*") {
        let base_dir = repo_path.join(pattern.trim_end_matches("/*"));
        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    results.push(entry.path());
                }
            }
        }
    }

    Ok(results)
}

mod node_common;
mod python_common;
mod ruby_common;

pub mod bun;
pub mod bundler;
pub mod cargo;
pub mod cmake;
pub mod composer;
pub mod dotnet;
pub mod go_mod;
pub mod gradle;
pub mod llm;
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
pub use llm::LLMBuildSystem;
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

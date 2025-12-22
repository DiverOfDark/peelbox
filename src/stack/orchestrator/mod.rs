//! Monorepo orchestrator definitions
//!
//! Orchestrators are tools that manage monorepo workspaces and coordinate
//! builds across multiple packages. They sit on top of build systems
//! (e.g., Turborepo works with npm/yarn/pnpm).

use anyhow::Result;
use std::path::PathBuf;

/// Package within a workspace
#[derive(Debug, Clone)]
pub struct Package {
    pub path: PathBuf,
    pub name: String,
    pub is_application: bool,
}

/// Complete workspace structure
#[derive(Debug, Clone)]
pub struct WorkspaceStructure {
    pub orchestrator: Option<OrchestratorId>,
    pub packages: Vec<Package>,
}

/// Monorepo orchestrator trait
pub trait MonorepoOrchestrator: Send + Sync {
    fn id(&self) -> OrchestratorId;

    /// Configuration files that indicate this orchestrator is in use
    fn config_files(&self) -> &[&str];

    /// Detect if this orchestrator is present based on file existence
    fn detect(&self, config_file: &str, content: Option<&str>) -> bool;

    /// Additional cache directories this orchestrator adds
    fn cache_dirs(&self) -> Vec<String>;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Parse workspace structure (PR9+)
    fn workspace_structure(&self, _repo_path: &std::path::Path) -> Result<WorkspaceStructure> {
        unimplemented!("workspace_structure not yet implemented for {}", self.name())
    }

    /// Generate build command for a package (PR9+)
    fn build_command(&self, _package: &Package) -> String {
        unimplemented!("build_command not yet implemented for {}", self.name())
    }
}

crate::define_id_enum! {
    /// Orchestrator identifier with support for LLM-discovered orchestrators
    OrchestratorId {
        Turborepo => "turborepo" : "Turborepo",
        Nx => "nx" : "Nx",
        Lerna => "lerna" : "Lerna",
        Rush => "rush" : "Rush",
    }
}

pub mod lerna;
pub mod nx;
pub mod turborepo;

pub use lerna::LernaOrchestrator;
pub use nx::NxOrchestrator;
pub use turborepo::TurborepoOrchestrator;

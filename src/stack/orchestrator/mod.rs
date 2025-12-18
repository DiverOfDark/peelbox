//! Monorepo orchestrator definitions
//!
//! Orchestrators are tools that manage monorepo workspaces and coordinate
//! builds across multiple packages. They sit on top of build systems
//! (e.g., Turborepo works with npm/yarn/pnpm).

use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrchestratorId {
    Turborepo,
    Nx,
    Lerna,
    Rush,
}

impl OrchestratorId {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Turborepo => "Turborepo",
            Self::Nx => "Nx",
            Self::Lerna => "Lerna",
            Self::Rush => "Rush",
        }
    }
}

pub mod lerna;
pub mod nx;
pub mod turborepo;

pub use lerna::LernaOrchestrator;
pub use nx::NxOrchestrator;
pub use turborepo::TurborepoOrchestrator;

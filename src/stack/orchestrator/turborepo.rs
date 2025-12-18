//! Turborepo orchestrator (Vercel)

use super::{MonorepoOrchestrator, OrchestratorId};

pub struct TurborepoOrchestrator;

impl MonorepoOrchestrator for TurborepoOrchestrator {
    fn id(&self) -> OrchestratorId {
        OrchestratorId::Turborepo
    }

    fn config_files(&self) -> &[&str] {
        &["turbo.json"]
    }

    fn detect(&self, config_file: &str, _content: Option<&str>) -> bool {
        config_file == "turbo.json"
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".turbo".to_string()]
    }

    fn name(&self) -> &'static str {
        "Turborepo"
    }
}

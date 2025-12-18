//! Nx orchestrator (Nrwl)

use super::{MonorepoOrchestrator, OrchestratorId};

pub struct NxOrchestrator;

impl MonorepoOrchestrator for NxOrchestrator {
    fn id(&self) -> OrchestratorId {
        OrchestratorId::Nx
    }

    fn config_files(&self) -> &[&str] {
        &["nx.json"]
    }

    fn detect(&self, config_file: &str, _content: Option<&str>) -> bool {
        config_file == "nx.json"
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".nx".to_string()]
    }

    fn name(&self) -> &'static str {
        "Nx"
    }
}

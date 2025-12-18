//! Lerna orchestrator

use super::{MonorepoOrchestrator, OrchestratorId};

pub struct LernaOrchestrator;

impl MonorepoOrchestrator for LernaOrchestrator {
    fn id(&self) -> OrchestratorId {
        OrchestratorId::Lerna
    }

    fn config_files(&self) -> &[&str] {
        &["lerna.json"]
    }

    fn detect(&self, config_file: &str, _content: Option<&str>) -> bool {
        config_file == "lerna.json"
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string()]
    }

    fn name(&self) -> &'static str {
        "Lerna"
    }
}

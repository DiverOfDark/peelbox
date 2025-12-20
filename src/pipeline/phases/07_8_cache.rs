use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use crate::pipeline::Confidence;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInfo {
    pub cache_dirs: Vec<PathBuf>,
    pub confidence: Confidence,
}

pub struct CachePhase;

#[async_trait]
impl ServicePhase for CachePhase {
    fn name(&self) -> &'static str {
        "CachePhase"
    }

    fn try_deterministic(&self, context: &mut ServiceContext) -> Result<Option<()>> {
        let build_system = context
            .stack_registry()
            .get_build_system(context.service.build_system)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown build system: {:?}",
                    context.service.build_system
                )
            })?;

        let cache_dirs = build_system
            .cache_dirs()
            .into_iter()
            .map(PathBuf::from)
            .collect();

        context.cache = Some(CacheInfo {
            cache_dirs,
            confidence: Confidence::High,
        });
        Ok(Some(()))
    }

    async fn execute_llm(&self, _context: &mut ServiceContext) -> Result<()> {
        anyhow::bail!(
            "CachePhase is always deterministic and should never call execute_llm. \
             This indicates a bug in the pipeline orchestration."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heuristics::HeuristicLogger;
    use crate::llm::MockLLMClient;
    use crate::pipeline::context::AnalysisContext;
    use crate::pipeline::phases::service_analysis::Service;
    use crate::stack::StackRegistry;
    use std::sync::Arc;

    async fn execute_phase(service: &Service) -> CacheInfo {
        use crate::config::DetectionMode;
        let llm_client: Arc<dyn crate::llm::LLMClient> = Arc::new(MockLLMClient::default());
        let stack_registry = Arc::new(StackRegistry::with_defaults());
        let heuristic_logger = Arc::new(HeuristicLogger::new(None));

        let analysis_context = AnalysisContext::new(
            &PathBuf::from("."),
            llm_client,
            stack_registry,
            None,
            heuristic_logger,
            DetectionMode::Full,
        );

        let service_arc = Arc::new(service.clone());
        let context_arc = Arc::new(analysis_context);
        let mut service_context = ServiceContext::new(service_arc, context_arc);
        let phase = CachePhase;
        phase.try_deterministic(&mut service_context).unwrap();
        service_context.cache.unwrap()
    }

    #[tokio::test]
    async fn test_cache_npm() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let result = execute_phase(&service).await;
        assert!(result.cache_dirs.contains(&PathBuf::from("node_modules")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[tokio::test]
    async fn test_cache_cargo() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        let result = execute_phase(&service).await;
        assert!(result.cache_dirs.contains(&PathBuf::from("target")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[tokio::test]
    async fn test_cache_maven() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "pom.xml".to_string(),
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Maven,
        };

        let result = execute_phase(&service).await;
        assert!(result.cache_dirs.contains(&PathBuf::from(".m2/repository")));
        assert!(result.cache_dirs.contains(&PathBuf::from("target")));
    }

    #[tokio::test]
    async fn test_cache_gradle() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "build.gradle".to_string(),
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Gradle,
        };

        let result = execute_phase(&service).await;
        assert!(result.cache_dirs.contains(&PathBuf::from(".gradle")));
        assert!(result.cache_dirs.contains(&PathBuf::from("build")));
    }

    #[tokio::test]
    async fn test_cache_go() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "go.mod".to_string(),
            language: crate::stack::LanguageId::Go,
            build_system: crate::stack::BuildSystemId::GoMod,
        };

        let result = execute_phase(&service).await;
        assert!(!result.cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }

    #[tokio::test]
    async fn test_cache_pipenv() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Pipfile".to_string(),
            language: crate::stack::LanguageId::Python,
            build_system: crate::stack::BuildSystemId::Pipenv,
        };

        let result = execute_phase(&service).await;
        // Pipenv may or may not have cache dirs depending on implementation
        assert_eq!(result.confidence, Confidence::High);
    }
}

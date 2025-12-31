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

    async fn execute(&self, context: &mut ServiceContext) -> Result<()> {
        let cache_dirs: Vec<PathBuf> = if let Some(build_system) = context
            .stack_registry()
            .get_build_system(context.service.build_system.clone())
        {
            build_system
                .cache_dirs()
                .into_iter()
                .map(PathBuf::from)
                .collect()
        } else {
            // Unknown/LLM-detected build system - no cache dirs
            vec![]
        };

        let is_empty = cache_dirs.is_empty();

        context.cache = Some(CacheInfo {
            cache_dirs,
            confidence: if is_empty {
                Confidence::Low
            } else {
                Confidence::High
            },
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heuristics::HeuristicLogger;
    use crate::pipeline::context::AnalysisContext;
    use crate::pipeline::phases::service_analysis::Service;
    use crate::stack::StackRegistry;
    use std::sync::Arc;

    async fn execute_phase(service: &Service) -> CacheInfo {
        use crate::config::DetectionMode;
        let stack_registry = Arc::new(StackRegistry::with_defaults(None));
        let wolfi_index = Arc::new(crate::validation::WolfiPackageIndex::for_tests());
        let heuristic_logger = Arc::new(HeuristicLogger::new(None));

        let analysis_context = AnalysisContext::new(
            &PathBuf::from("."),
            stack_registry,
            wolfi_index,
            None,
            heuristic_logger,
            DetectionMode::Full,
        );

        let service_arc = Arc::new(service.clone());
        let context_arc = Arc::new(analysis_context);
        let mut service_context = ServiceContext::new(service_arc, context_arc);
        let phase = CachePhase;
        phase.execute(&mut service_context).await.unwrap();
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

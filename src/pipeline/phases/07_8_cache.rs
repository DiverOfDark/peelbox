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

    type Output = CacheInfo;

    async fn execute(&self, context: &ServiceContext) -> Result<CacheInfo> {
        use crate::stack::BuildSystemId;
        let cache_dirs = match context.service.build_system {
        BuildSystemId::Npm | BuildSystemId::Yarn | BuildSystemId::Pnpm | BuildSystemId::Bun => {
            vec![
                PathBuf::from("node_modules"),
                PathBuf::from(".npm"),
                PathBuf::from(".pnpm-store"),
                PathBuf::from(".yarn/cache"),
            ]
        }
        BuildSystemId::Cargo => vec![PathBuf::from("target")],
        BuildSystemId::Maven => vec![PathBuf::from(".m2/repository"), PathBuf::from("target")],
        BuildSystemId::Gradle => vec![PathBuf::from(".gradle"), PathBuf::from("build")],
        BuildSystemId::GoMod => vec![PathBuf::from("go/pkg/mod")],
        BuildSystemId::Pip | BuildSystemId::Poetry => vec![
            PathBuf::from("__pycache__"),
            PathBuf::from(".venv"),
            PathBuf::from("venv"),
        ],
        BuildSystemId::Composer => vec![PathBuf::from("vendor")],
        BuildSystemId::Bundler => vec![PathBuf::from("vendor/bundle")],
        BuildSystemId::Mix => vec![PathBuf::from("_build"), PathBuf::from("deps")],
        BuildSystemId::DotNet => vec![PathBuf::from("obj"), PathBuf::from("bin")],
        BuildSystemId::CMake => vec![PathBuf::from("build")],
        BuildSystemId::Make => vec![],
        BuildSystemId::Meson => vec![PathBuf::from("builddir")],
        BuildSystemId::Pipenv => vec![],
    };

        let result = CacheInfo {
            cache_dirs,
            confidence: Confidence::High,
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heuristics::HeuristicLogger;
    use crate::llm::MockLLMClient;
    use crate::pipeline::context::AnalysisContext;
    use crate::pipeline::phases::structure::Service;
    use crate::stack::StackRegistry;
    use std::sync::Arc;

    async fn execute_phase(service: &Service) -> CacheInfo {
        let llm_client: Arc<dyn crate::llm::LLMClient> = Arc::new(MockLLMClient::default());
        let stack_registry = Arc::new(StackRegistry::with_defaults());
        let heuristic_logger = Arc::new(HeuristicLogger::new(None));

        let analysis_context = AnalysisContext::new(
            &PathBuf::from("."),
            llm_client,
            stack_registry,
            None,
            heuristic_logger,
        );

        let service_context = ServiceContext::new(service, &analysis_context);
        let phase = CachePhase;
        phase.execute(&service_context).await.unwrap()
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
        assert_eq!(result.cache_dirs, vec![PathBuf::from("target")]);
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
        assert_eq!(result.cache_dirs, vec![PathBuf::from("go/pkg/mod")]);
    }

    #[tokio::test]
    async fn test_cache_unknown() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "unknown.txt".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Pipenv,
        };

        let result = execute_phase(&service).await;
        assert!(result.cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }
}

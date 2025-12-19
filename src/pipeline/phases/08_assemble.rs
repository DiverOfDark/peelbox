use super::root_cache::RootCacheInfo;
use crate::pipeline::service_context::ServiceContext;
use crate::output::schema::{
    BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage, UniversalBuild,
};
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::stack::registry::StackRegistry;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct AssemblePhase;

#[async_trait]
impl WorkflowPhase for AssemblePhase {
    fn name(&self) -> &'static str {
        "AssemblePhase"
    }

    fn try_deterministic(&self, context: &mut AnalysisContext) -> Result<Option<()>> {
        let root_cache = context
            .root_cache
            .as_ref()
            .expect("Root cache must be available before assemble");

        let builds = execute_assemble(
            &context.service_analyses,
            root_cache,
            &context.stack_registry,
        )?;

        context.builds = builds;
        Ok(Some(()))
    }

    async fn execute_llm(&self, context: &mut AnalysisContext) -> Result<()> {
        let root_cache = context
            .root_cache
            .as_ref()
            .expect("Root cache must be available before assemble");

        let builds = execute_assemble(
            &context.service_analyses,
            root_cache,
            &context.stack_registry,
        )?;
        context.builds = builds;
        Ok(())
    }
}

fn execute_assemble(
    analysis_results: &[ServiceContext],
    root_cache: &RootCacheInfo,
    registry: &std::sync::Arc<StackRegistry>,
) -> Result<Vec<UniversalBuild>> {
    let mut builds = Vec::new();

    for result in analysis_results {
        let build = assemble_single_service(result, root_cache, &registry)?;
        builds.push(build);
    }

    Ok(builds)
}

fn assemble_single_service(
    result: &ServiceContext,
    root_cache: &RootCacheInfo,
    registry: &StackRegistry,
) -> Result<UniversalBuild> {
    let _language_def = registry.get_language(result.service.language);

    let template = registry
        .get_build_system(result.service.build_system)
        .map(|bs| bs.build_template());

    let project_name = extract_project_name(&result.service);

    let confidence = calculate_confidence(result);

    let runtime = result.runtime.as_ref().expect("Runtime must be set");
    let build_info = result.build.as_ref().expect("Build must be set");
    let cache_info = result.cache.as_ref().expect("Cache must be set");
    let env_vars_info = result.env_vars.as_ref().expect("EnvVars must be set");
    let entrypoint_info = result.entrypoint.as_ref().expect("Entrypoint must be set");
    let port_info = result.port.as_ref().expect("Port must be set");

    let metadata = BuildMetadata {
        project_name: Some(project_name.clone()),
        language: result.service.language.name().to_string(),
        build_system: result.service.build_system.name().to_string(),
        framework: runtime.framework.clone(),
        confidence,
        reasoning: format!(
            "Detected from {} in {}",
            result.service.manifest,
            result.service.path.display()
        ),
    };

    let mut cache_paths: Vec<String> = cache_info
        .cache_dirs
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    cache_paths.extend(
        root_cache
            .root_cache_dirs
            .iter()
            .map(|p| p.display().to_string()),
    );

    let build = BuildStage {
        base: template
            .as_ref()
            .map(|t| t.build_image.clone())
            .unwrap_or_else(|| format!("{}:latest", runtime.runtime)),
        packages: template
            .as_ref()
            .map(|t| t.build_packages.clone())
            .unwrap_or_default(),
        env: HashMap::new(),
        commands: build_info.build_cmd.clone().into_iter().collect::<Vec<_>>(),
        context: vec![ContextSpec {
            from: result.service.path.display().to_string(),
            to: "/app".to_string(),
        }],
        cache: cache_paths,
        artifacts: template
            .as_ref()
            .map(|t| {
                t.artifacts
                    .iter()
                    .map(|a| a.replace("{project_name}", &project_name))
                    .collect()
            })
            .unwrap_or_default(),
    };

    let mut env_map = HashMap::new();
    for env_var in &env_vars_info.env_vars {
        if let Some(default) = &env_var.default_value {
            env_map.insert(env_var.name.clone(), default.clone());
        }
    }

    let entrypoint_cmd = entrypoint_info
        .entrypoint
        .replace("{project_name}", &project_name);
    let command_parts: Vec<String> = entrypoint_cmd
        .split_whitespace()
        .map(String::from)
        .collect();

    let runtime = RuntimeStage {
        base: template
            .as_ref()
            .map(|t| t.runtime_image.clone())
            .unwrap_or_else(|| "debian:bookworm-slim".to_string()),
        packages: template
            .as_ref()
            .map(|t| t.runtime_packages.clone())
            .unwrap_or_default(),
        env: env_map,
        copy: vec![CopySpec {
            from: build
                .artifacts
                .first()
                .cloned()
                .unwrap_or_else(|| "/app".to_string()),
            to: "/usr/local/bin/app".to_string(),
        }],
        command: command_parts,
        ports: port_info.port.into_iter().collect(),
    };

    Ok(UniversalBuild {
        version: "1.0".to_string(),
        metadata,
        build,
        runtime,
    })
}

fn extract_project_name(service: &super::structure::Service) -> String {
    service
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app")
        .to_string()
}

fn calculate_confidence(result: &ServiceContext) -> f32 {
    let mut scores = [
        result.runtime.as_ref().expect("Runtime must be set").confidence.to_f32(),
        result.build.as_ref().expect("Build must be set").confidence.to_f32(),
        result.entrypoint.as_ref().expect("Entrypoint must be set").confidence.to_f32(),
        result.native_deps.as_ref().expect("NativeDeps must be set").confidence.to_f32(),
        result.port.as_ref().expect("Port must be set").confidence.to_f32(),
        result.env_vars.as_ref().expect("EnvVars must be set").confidence.to_f32(),
        result.health.as_ref().expect("Health must be set").confidence.to_f32(),
        result.cache.as_ref().expect("Cache must be set").confidence.to_f32(),
    ];

    scores.sort_by(|a, b| b.partial_cmp(a).unwrap());

    scores.iter().take(5).sum::<f32>() / 5.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::build::BuildInfo;
    use crate::pipeline::phases::cache::CacheInfo;
    use crate::pipeline::phases::entrypoint::EntrypointInfo;
    use crate::pipeline::phases::env_vars::EnvVarsInfo;
    use crate::pipeline::phases::health::HealthInfo;
    use crate::pipeline::phases::native_deps::NativeDepsInfo;
    use crate::pipeline::phases::port::PortInfo;
    use crate::pipeline::phases::runtime::RuntimeInfo;
    use crate::pipeline::phases::structure::Service;
    use crate::pipeline::Confidence;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_extract_project_name() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        assert_eq!(extract_project_name(&service), "web");
    }

    #[test]
    fn test_extract_project_name_root() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        assert_eq!(extract_project_name(&service), "app");
    }

    #[test]
    fn test_confidence_calculation() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let llm_client: Arc<dyn crate::llm::LLMClient> = Arc::new(crate::llm::MockLLMClient::default());
        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let heuristic_logger = Arc::new(crate::heuristics::HeuristicLogger::new(None));

        let analysis_context = crate::pipeline::context::AnalysisContext::new(
            &PathBuf::from("."),
            llm_client,
            stack_registry,
            None,
            heuristic_logger,
            crate::config::DetectionMode::Full,
        );

        let result = ServiceContext {
            service: Arc::new(service),
            analysis_context: Arc::new(analysis_context),
            runtime: Some(RuntimeInfo {
                runtime: "node".to_string(),
                runtime_version: None,
                framework: None,
                confidence: Confidence::High,
            }),
            build: Some(BuildInfo {
                build_cmd: Some("npm run build".to_string()),
                output_dir: Some(PathBuf::from("dist")),
                confidence: Confidence::High,
            }),
            entrypoint: Some(EntrypointInfo {
                entrypoint: "node dist/main.js".to_string(),
                confidence: Confidence::High,
            }),
            native_deps: Some(NativeDepsInfo {
                needs_build_deps: false,
                has_native_modules: false,
                has_prisma: false,
                native_deps: vec![],
                confidence: Confidence::High,
            }),
            port: Some(PortInfo {
                port: Some(3000),
                from_env: false,
                env_var: None,
                confidence: Confidence::High,
            }),
            env_vars: Some(EnvVarsInfo {
                env_vars: vec![],
                confidence: Confidence::High,
            }),
            health: Some(HealthInfo {
                health_endpoints: vec![],
                recommended_liveness: None,
                recommended_readiness: None,
                confidence: Confidence::High,
            }),
            cache: Some(CacheInfo {
                cache_dirs: vec![],
                confidence: Confidence::High,
            }),
        };

        let confidence = calculate_confidence(&result);
        assert!(confidence >= 0.9);
    }
}

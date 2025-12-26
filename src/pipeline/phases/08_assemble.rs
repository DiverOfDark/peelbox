use super::root_cache::RootCacheInfo;
use crate::output::schema::{
    BuildMetadata, BuildStage, CopySpec, RuntimeStage, UniversalBuild,
};
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::pipeline::service_context::ServiceContext;
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

    async fn execute(&self, context: &mut AnalysisContext) -> Result<()> {
        let root_cache = context
            .root_cache
            .as_ref()
            .expect("Root cache must be available before assemble");

        let builds = execute_assemble(
            &context.service_analyses,
            root_cache,
            &context.stack_registry,
            &context.wolfi_index,
        )?;
        context.builds = builds;
        Ok(())
    }
}

fn execute_assemble(
    analysis_results: &[ServiceContext],
    root_cache: &RootCacheInfo,
    registry: &std::sync::Arc<StackRegistry>,
    wolfi_index: &std::sync::Arc<crate::validation::WolfiPackageIndex>,
) -> Result<Vec<UniversalBuild>> {
    let mut builds = Vec::new();

    for result in analysis_results {
        let build = assemble_single_service(result, root_cache, registry, wolfi_index)?;
        builds.push(build);
    }

    Ok(builds)
}

fn assemble_single_service(
    result: &ServiceContext,
    root_cache: &RootCacheInfo,
    registry: &StackRegistry,
    wolfi_index: &crate::validation::WolfiPackageIndex,
) -> Result<UniversalBuild> {
    let _language_def = registry.get_language(result.service.language.clone());

    // Read manifest content for version parsing
    let service_path = result.repo_path().join(&result.service.path);
    let manifest_path = service_path.join(&result.service.manifest);
    let manifest_content = std::fs::read_to_string(&manifest_path).ok();

    let template = registry
        .get_build_system(result.service.build_system.clone())
        .map(|bs| bs.build_template(wolfi_index, &service_path, manifest_content.as_deref()));

    let project_name = extract_project_name(&result.service);

    let confidence = calculate_confidence(result);

    let stack = result.stack.as_ref().expect("Stack must be set");
    let build_info = result.build.as_ref().expect("Build must be set");
    let cache_info = result.cache.as_ref().expect("Cache must be set");

    // Extract from runtime_config with defaults
    let runtime_config = result.runtime_config.as_ref();
    let entrypoint_cmd = runtime_config
        .and_then(|rc| rc.entrypoint.clone())
        .unwrap_or_else(|| "bin/app".to_string());
    let port = runtime_config
        .and_then(|rc| rc.port)
        .or_else(|| {
            registry
                .get_language(result.service.language.clone())
                .and_then(|lang| lang.default_port())
        })
        .unwrap_or(8080);
    let _env_vars = runtime_config
        .map(|rc| &rc.env_vars)
        .cloned()
        .unwrap_or_default();
    let _native_deps = runtime_config
        .map(|rc| &rc.native_deps)
        .cloned()
        .unwrap_or_default();

    let metadata = BuildMetadata {
        project_name: Some(project_name.clone()),
        language: stack.language.name().to_string(),
        build_system: stack.build_system.name().to_string(),
        framework: stack.framework.as_ref().map(|fw| fw.name().to_string()),
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
        packages: template
            .as_ref()
            .map(|t| t.build_packages.clone())
            .unwrap_or_default(),
        env: HashMap::new(),
        commands: build_info.build_cmd.clone().into_iter().collect::<Vec<_>>(),
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

    // Build env map from runtime_config env_vars (simplified - just var names, no defaults for now)
    let env_map = HashMap::new(); // TODO: Parse env_vars into key=value pairs

    let entrypoint_replaced = entrypoint_cmd.replace("{project_name}", &project_name);
    let command_parts: Vec<String> = entrypoint_replaced
        .split_whitespace()
        .map(String::from)
        .collect();

    let runtime_packages = {
        let runtime = registry.get_runtime(stack.runtime.clone(), None);
        runtime.runtime_packages(wolfi_index, &service_path, manifest_content.as_deref())
    };

    let runtime = RuntimeStage {
        packages: runtime_packages,
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
        ports: vec![port],
        health: runtime_config.and_then(|rc| rc.health.clone()),
    };

    Ok(UniversalBuild {
        version: "1.0".to_string(),
        metadata,
        build,
        runtime,
    })
}

fn extract_project_name(service: &super::service_analysis::Service) -> String {
    service
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app")
        .to_string()
}

fn calculate_confidence(result: &ServiceContext) -> f32 {
    // Stack detection is always High confidence (deterministic)
    let stack_confidence = crate::pipeline::Confidence::High.to_f32();

    let mut scores = [
        stack_confidence,
        result
            .build
            .as_ref()
            .expect("Build must be set")
            .confidence
            .to_f32(),
        result
            .cache
            .as_ref()
            .expect("Cache must be set")
            .confidence
            .to_f32(),
    ];

    scores.sort_by(|a, b| b.partial_cmp(a).unwrap());

    scores.iter().sum::<f32>() / scores.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::build::BuildInfo;
    use crate::pipeline::phases::cache::CacheInfo;
    use crate::pipeline::phases::service_analysis::Service;
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

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults(None));
        let wolfi_index = Arc::new(crate::validation::WolfiPackageIndex::for_tests());
        let heuristic_logger = Arc::new(crate::heuristics::HeuristicLogger::new(None));

        let analysis_context = crate::pipeline::context::AnalysisContext::new(
            &PathBuf::from("."),
            stack_registry,
            wolfi_index,
            None,
            heuristic_logger,
            crate::config::DetectionMode::Full,
        );

        let result = ServiceContext {
            service: Arc::new(service),
            analysis_context: Arc::new(analysis_context),
            stack: Some(crate::pipeline::service_context::Stack {
                language: crate::stack::LanguageId::JavaScript,
                build_system: crate::stack::BuildSystemId::Npm,
                framework: None,
                runtime: crate::stack::RuntimeId::Node,
                version: None,
            }),
            runtime_config: None,
            build: Some(BuildInfo {
                build_cmd: Some("npm run build".to_string()),
                output_dir: Some(PathBuf::from("dist")),
                confidence: Confidence::High,
            }),
            cache: Some(CacheInfo {
                cache_dirs: vec![],
                confidence: Confidence::High,
            }),
        };

        let confidence = calculate_confidence(&result);
        assert!(confidence >= 0.8);
    }
}

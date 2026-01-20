use super::root_cache::RootCacheInfo;
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::pipeline::service_context::ServiceContext;
use anyhow::Result;
use async_trait::async_trait;
use peelbox_core::output::schema::{
    BuildMetadata, BuildStage, CopySpec, RuntimeStage, UniversalBuild,
};
use peelbox_stack::registry::StackRegistry;
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
    wolfi_index: &std::sync::Arc<peelbox_wolfi::WolfiPackageIndex>,
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
    wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
) -> Result<UniversalBuild> {
    let _language_def = registry.get_language(result.service.language.clone());

    // Read manifest content for version parsing
    let service_path = result.repo_path().join(&result.service.path);
    let manifest_path = service_path.join(&result.service.manifest);
    let manifest_content = std::fs::read_to_string(&manifest_path).ok();

    let build_system = registry.get_build_system(result.service.build_system.clone());

    let template = build_system
        .as_ref()
        .map(|bs| bs.build_template(wolfi_index, &service_path, manifest_content.as_deref()));

    let project_name = manifest_content
        .as_deref()
        .and_then(|content| {
            build_system
                .as_ref()
                .and_then(|bs| bs.parse_package_metadata(content).ok())
                .map(|(name, _)| name)
        })
        .unwrap_or_else(|| {
            // Fall back to directory name
            let path = &result.service.path;
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("app")
                .to_string()
        });

    let stack = result.stack.as_ref().expect("Stack must be set");
    let build_info = result.build.as_ref().expect("Build must be set");
    let cache_info = result.cache.as_ref().expect("Cache must be set");

    // Extract from runtime_config with defaults
    let runtime_config = result.runtime_config.as_ref();
    let entrypoint_cmd = runtime_config
        .and_then(|rc| rc.entrypoint.clone())
        .unwrap_or_else(|| "/usr/local/bin/{project_name}".to_string());
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
        env: template
            .as_ref()
            .map(|t| t.build_env.clone())
            .unwrap_or_default(),
        commands: build_info.build_cmd.clone(),
        cache: cache_paths,
    };

    let mut env_map = HashMap::new();

    // Add build system runtime environment variables
    if let Some(ref tmpl) = template {
        env_map.extend(tmpl.runtime_env.clone());
    }

    // Add framework-specific runtime environment variables
    if let Some(framework_id) = &stack.framework {
        if let Some(framework) = registry.get_framework(framework_id.clone()) {
            env_map.extend(framework.runtime_env_vars());
        }
    }

    let entrypoint_replaced = entrypoint_cmd.replace("{project_name}", &project_name);
    let command_parts: Vec<String> = entrypoint_replaced
        .split_whitespace()
        .map(String::from)
        .collect();

    let runtime_instance = registry.get_runtime(stack.runtime.clone(), None);

    let runtime_packages =
        runtime_instance.runtime_packages(wolfi_index, &service_path, manifest_content.as_deref());

    env_map.extend(runtime_instance.runtime_env(
        wolfi_index,
        &service_path,
        manifest_content.as_deref(),
    ));

    let runtime_copy = template
        .as_ref()
        .map(|t| {
            t.runtime_copy
                .iter()
                .map(|(from, to)| CopySpec {
                    from: from.replace("{project_name}", &project_name),
                    to: to.replace("{project_name}", &project_name),
                })
                .collect()
        })
        .unwrap_or_default();

    let runtime = RuntimeStage {
        packages: runtime_packages,
        env: env_map,
        copy: runtime_copy,
        command: command_parts,
        workdir: "/app".to_string(),
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
    fn test_confidence_calculation() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: peelbox_stack::LanguageId::JavaScript,
            build_system: peelbox_stack::BuildSystemId::Npm,
        };

        let stack_registry = Arc::new(peelbox_stack::StackRegistry::with_defaults(None));
        let wolfi_index = Arc::new(peelbox_wolfi::WolfiPackageIndex::for_tests());

        let analysis_context = crate::pipeline::context::AnalysisContext::new(
            &PathBuf::from("."),
            stack_registry,
            wolfi_index,
            peelbox_core::config::DetectionMode::Full,
        );

        let _result = ServiceContext {
            service: Arc::new(service),
            analysis_context: Arc::new(analysis_context),
            stack: Some(crate::pipeline::service_context::Stack {
                language: peelbox_stack::LanguageId::JavaScript,
                build_system: peelbox_stack::BuildSystemId::Npm,
                framework: None,
                runtime: peelbox_stack::RuntimeId::Node,
                version: None,
            }),
            runtime_config: None,
            build: Some(BuildInfo {
                build_cmd: vec!["npm run build".to_string()],
                output_dir: Some(PathBuf::from("dist")),
                confidence: Confidence::High,
            }),
            cache: Some(CacheInfo {
                cache_dirs: vec![],
                confidence: Confidence::High,
            }),
        };
    }
}

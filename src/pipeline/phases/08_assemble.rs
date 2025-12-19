use super::build::BuildInfo;
use super::cache::CacheInfo;
use super::entrypoint::EntrypointInfo;
use super::env_vars::EnvVarsInfo;
use super::health::HealthInfo;
use super::native_deps::NativeDepsInfo;
use super::port::PortInfo;
use super::root_cache::RootCacheInfo;
use super::runtime::RuntimeInfo;
use super::structure::Service;
use crate::output::schema::{
    BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage, UniversalBuild,
};
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::stack::registry::StackRegistry;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct ServiceAnalysisResults {
    pub service: Service,
    pub runtime: RuntimeInfo,
    pub build: BuildInfo,
    pub entrypoint: EntrypointInfo,
    pub native_deps: NativeDepsInfo,
    pub port: PortInfo,
    pub env_vars: EnvVarsInfo,
    pub health: HealthInfo,
    pub cache: CacheInfo,
}

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
        )?;
        context.builds = builds;
        Ok(())
    }
}

fn execute_assemble(
    analysis_results: &[ServiceAnalysisResults],
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
    result: &ServiceAnalysisResults,
    root_cache: &RootCacheInfo,
    registry: &StackRegistry,
) -> Result<UniversalBuild> {
    let _language_def = registry.get_language(result.service.language);

    let template = registry
        .get_build_system(result.service.build_system)
        .map(|bs| bs.build_template());

    let project_name = extract_project_name(&result.service);

    let confidence = calculate_confidence(result);

    let metadata = BuildMetadata {
        project_name: Some(project_name.clone()),
        language: result.service.language.name().to_string(),
        build_system: result.service.build_system.name().to_string(),
        framework: result.runtime.framework.clone(),
        confidence,
        reasoning: format!(
            "Detected from {} in {}",
            result.service.manifest,
            result.service.path.display()
        ),
    };

    let mut cache_paths: Vec<String> = result
        .cache
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
            .unwrap_or_else(|| format!("{}:latest", result.runtime.runtime)),
        packages: template
            .as_ref()
            .map(|t| t.build_packages.clone())
            .unwrap_or_default(),
        env: HashMap::new(),
        commands: result.build.build_cmd.clone().into_iter().collect::<Vec<_>>(),
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
    for env_var in &result.env_vars.env_vars {
        if let Some(default) = &env_var.default_value {
            env_map.insert(env_var.name.clone(), default.clone());
        }
    }

    let entrypoint_cmd = result
        .entrypoint
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
        ports: result.port.port.into_iter().collect(),
    };

    Ok(UniversalBuild {
        version: "1.0".to_string(),
        metadata,
        build,
        runtime,
    })
}

fn extract_project_name(service: &Service) -> String {
    service
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app")
        .to_string()
}

fn calculate_confidence(result: &ServiceAnalysisResults) -> f32 {
    let mut scores = [
        result.runtime.confidence.to_f32(),
        result.build.confidence.to_f32(),
        result.entrypoint.confidence.to_f32(),
        result.native_deps.confidence.to_f32(),
        result.port.confidence.to_f32(),
        result.env_vars.confidence.to_f32(),
        result.health.confidence.to_f32(),
        result.cache.confidence.to_f32(),
    ];

    scores.sort_by(|a, b| b.partial_cmp(a).unwrap());

    scores.iter().take(5).sum::<f32>() / 5.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::Confidence;
    use std::path::PathBuf;

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

        let result = ServiceAnalysisResults {
            service,
            runtime: RuntimeInfo {
                runtime: "node".to_string(),
                runtime_version: None,
                framework: None,
                confidence: Confidence::High,
            },
            build: BuildInfo {
                build_cmd: Some("npm run build".to_string()),
                output_dir: Some(PathBuf::from("dist")),
                confidence: Confidence::High,
            },
            entrypoint: EntrypointInfo {
                entrypoint: "node dist/main.js".to_string(),
                confidence: Confidence::High,
            },
            native_deps: NativeDepsInfo {
                needs_build_deps: false,
                has_native_modules: false,
                has_prisma: false,
                native_deps: vec![],
                confidence: Confidence::High,
            },
            port: PortInfo {
                port: Some(3000),
                from_env: false,
                env_var: None,
                confidence: Confidence::High,
            },
            env_vars: EnvVarsInfo {
                env_vars: vec![],
                confidence: Confidence::High,
            },
            health: HealthInfo {
                health_endpoints: vec![],
                recommended_liveness: None,
                recommended_readiness: None,
                confidence: Confidence::High,
            },
            cache: CacheInfo {
                cache_dirs: vec![],
                confidence: Confidence::High,
            },
        };

        let confidence = calculate_confidence(&result);
        assert!(confidence >= 0.9);
    }
}

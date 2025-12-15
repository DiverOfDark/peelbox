use super::build::BuildInfo;
use super::cache::CacheInfo;
use super::entrypoint::EntrypointInfo;
use super::env_vars::EnvVarsInfo;
use super::health::HealthInfo;
use super::native_deps::NativeDepsInfo;
use super::port::PortInfo;
use super::root_cache::RootCacheInfo;
use super::runtime::RuntimeInfo;
use super::structure::{Service, StructureResult};
use crate::languages::LanguageRegistry;
use crate::output::schema::{
    BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage, UniversalBuild,
};
use anyhow::Result;
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

pub fn execute(
    analysis_results: Vec<ServiceAnalysisResults>,
    _structure: &StructureResult,
    root_cache: &RootCacheInfo,
) -> Result<Vec<UniversalBuild>> {
    let mut builds = Vec::new();

    for result in analysis_results {
        let build = assemble_single_service(result, root_cache)?;
        builds.push(build);
    }

    Ok(builds)
}

fn assemble_single_service(
    result: ServiceAnalysisResults,
    root_cache: &RootCacheInfo,
) -> Result<UniversalBuild> {
    let registry = LanguageRegistry::new();
    let language_def = registry.get_by_name(&result.service.language);

    let template = language_def.and_then(|def| def.build_template(&result.service.build_system));

    let project_name = extract_project_name(&result.service);

    let confidence = calculate_confidence(&result);

    let metadata = BuildMetadata {
        project_name: Some(project_name.clone()),
        language: result.service.language.clone(),
        build_system: result.service.build_system.clone(),
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
        commands: result
            .build
            .build_cmd
            .into_iter()
            .collect::<Vec<_>>(),
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
    let command_parts: Vec<String> = entrypoint_cmd.split_whitespace().map(String::from).collect();

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
    let mut scores = vec![
        confidence_to_f32(result.runtime.confidence),
        confidence_to_f32(result.build.confidence),
        confidence_to_f32(result.entrypoint.confidence),
        confidence_to_f32(result.native_deps.confidence),
        confidence_to_f32(result.port.confidence),
        confidence_to_f32(result.env_vars.confidence),
        confidence_to_f32(result.health.confidence),
        confidence_to_f32(result.cache.confidence),
    ];

    scores.sort_by(|a, b| b.partial_cmp(a).unwrap());

    scores.iter().take(5).sum::<f32>() / 5.0
}

fn confidence_to_f32<T>(conf: T) -> f32
where
    T: Into<ConfidenceValue>,
{
    match conf.into() {
        ConfidenceValue::High => 0.95,
        ConfidenceValue::Medium => 0.7,
        ConfidenceValue::Low => 0.4,
    }
}

enum ConfidenceValue {
    High,
    Medium,
    Low,
}

impl From<super::runtime::Confidence> for ConfidenceValue {
    fn from(c: super::runtime::Confidence) -> Self {
        match c {
            super::runtime::Confidence::High => ConfidenceValue::High,
            super::runtime::Confidence::Medium => ConfidenceValue::Medium,
            super::runtime::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::build::Confidence> for ConfidenceValue {
    fn from(c: super::build::Confidence) -> Self {
        match c {
            super::build::Confidence::High => ConfidenceValue::High,
            super::build::Confidence::Medium => ConfidenceValue::Medium,
            super::build::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::entrypoint::Confidence> for ConfidenceValue {
    fn from(c: super::entrypoint::Confidence) -> Self {
        match c {
            super::entrypoint::Confidence::High => ConfidenceValue::High,
            super::entrypoint::Confidence::Medium => ConfidenceValue::Medium,
            super::entrypoint::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::native_deps::Confidence> for ConfidenceValue {
    fn from(c: super::native_deps::Confidence) -> Self {
        match c {
            super::native_deps::Confidence::High => ConfidenceValue::High,
            super::native_deps::Confidence::Medium => ConfidenceValue::Medium,
            super::native_deps::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::port::Confidence> for ConfidenceValue {
    fn from(c: super::port::Confidence) -> Self {
        match c {
            super::port::Confidence::High => ConfidenceValue::High,
            super::port::Confidence::Medium => ConfidenceValue::Medium,
            super::port::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::env_vars::Confidence> for ConfidenceValue {
    fn from(c: super::env_vars::Confidence) -> Self {
        match c {
            super::env_vars::Confidence::High => ConfidenceValue::High,
            super::env_vars::Confidence::Medium => ConfidenceValue::Medium,
            super::env_vars::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::health::Confidence> for ConfidenceValue {
    fn from(c: super::health::Confidence) -> Self {
        match c {
            super::health::Confidence::High => ConfidenceValue::High,
            super::health::Confidence::Medium => ConfidenceValue::Medium,
            super::health::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

impl From<super::cache::Confidence> for ConfidenceValue {
    fn from(c: super::cache::Confidence) -> Self {
        match c {
            super::cache::Confidence::High => ConfidenceValue::High,
            super::cache::Confidence::Medium => ConfidenceValue::Medium,
            super::cache::Confidence::Low => ConfidenceValue::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_project_name() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        assert_eq!(extract_project_name(&service), "web");
    }

    #[test]
    fn test_extract_project_name_root() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
        };

        assert_eq!(extract_project_name(&service), ".");
    }

    #[test]
    fn test_confidence_calculation() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let result = ServiceAnalysisResults {
            service,
            runtime: RuntimeInfo {
                runtime: "node".to_string(),
                runtime_version: None,
                framework: None,
                confidence: super::runtime::Confidence::High,
            },
            build: BuildInfo {
                build_cmd: Some("npm run build".to_string()),
                output_dir: Some(PathBuf::from("dist")),
                confidence: super::build::Confidence::High,
            },
            entrypoint: EntrypointInfo {
                entrypoint: "node dist/main.js".to_string(),
                confidence: super::entrypoint::Confidence::High,
            },
            native_deps: NativeDepsInfo {
                needs_build_deps: false,
                has_native_modules: false,
                has_prisma: false,
                native_deps: vec![],
                confidence: super::native_deps::Confidence::High,
            },
            port: PortInfo {
                port: Some(3000),
                from_env: false,
                env_var: None,
                confidence: super::port::Confidence::High,
            },
            env_vars: EnvVarsInfo {
                env_vars: vec![],
                confidence: super::env_vars::Confidence::High,
            },
            health: HealthInfo {
                health_endpoints: vec![],
                recommended_liveness: None,
                recommended_readiness: None,
                confidence: super::health::Confidence::High,
            },
            cache: CacheInfo {
                cache_dirs: vec![],
                confidence: super::cache::Confidence::High,
            },
        };

        let confidence = calculate_confidence(&result);
        assert!(confidence >= 0.9);
    }
}

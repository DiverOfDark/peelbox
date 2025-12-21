use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::{ServiceContext, Stack};
use crate::stack::{FrameworkId, LanguageId, RuntimeId, StackRegistry};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

pub struct StackIdentificationPhase;

#[async_trait]
impl ServicePhase for StackIdentificationPhase {
    fn name(&self) -> &'static str {
        "StackIdentificationPhase"
    }

    fn try_deterministic(&self, context: &mut ServiceContext) -> Result<Option<()>> {
        if let Some(stack) = try_detect_stack(
            context.service.language,
            context.service.build_system,
            &context.service.path,
            &context.service.manifest,
            context.repo_path(),
            context.stack_registry(),
        ) {
            context.stack = Some(stack);
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    async fn execute_llm(&self, _context: &mut ServiceContext) -> Result<()> {
        // LLM fallback not yet implemented - use deterministic detection only
        Ok(())
    }
}

fn try_detect_stack(
    language: LanguageId,
    build_system: crate::stack::BuildSystemId,
    service_path: &PathBuf,
    manifest_name: &str,
    repo_path: &std::path::Path,
    stack_registry: &Arc<StackRegistry>,
) -> Option<Stack> {
    let language_def = stack_registry.get_language(language)?;
    let runtime_name = language_def.runtime_name()?;
    let runtime = RuntimeId::from_name(runtime_name)?;

    let framework = detect_framework(service_path, manifest_name, repo_path, stack_registry);

    Some(Stack {
        language,
        build_system,
        framework,
        runtime,
        version: None,
    })
}

fn detect_framework(
    service_path: &PathBuf,
    manifest_name: &str,
    repo_path: &std::path::Path,
    stack_registry: &Arc<StackRegistry>,
) -> Option<FrameworkId> {
    let manifest_path = repo_path.join(service_path).join(manifest_name);
    let manifest_content = std::fs::read_to_string(&manifest_path).ok()?;

    // Parse dependencies from manifest using stack registry
    let dep_info = stack_registry.parse_dependencies_by_manifest(
        manifest_name,
        &manifest_content,
        std::slice::from_ref(service_path),
    )?;

    // Try to match framework dependency patterns
    for fw_id in FrameworkId::all_variants() {
        if let Some(fw) = stack_registry.get_framework(*fw_id) {
            let patterns = fw.dependency_patterns();
            for pattern in &patterns {
                if dep_info.external_deps.iter().any(|d| pattern.matches(d))
                    || dep_info.internal_deps.iter().any(|d| pattern.matches(d))
                {
                    return Some(*fw_id);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::service_analysis::Service;

    #[test]
    fn test_detect_stack_rust() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        let stack_registry = Arc::new(StackRegistry::with_defaults());
        let repo_path = PathBuf::from(".");

        let stack = try_detect_stack(
            service.language,
            service.build_system,
            &service.path,
            &service.manifest,
            &repo_path,
            &stack_registry,
        )
        .unwrap();

        assert_eq!(stack.language, LanguageId::Rust);
        assert_eq!(stack.build_system, crate::stack::BuildSystemId::Cargo);
        assert_eq!(stack.runtime, RuntimeId::Native);
        assert_eq!(stack.framework, None);
    }

    #[test]
    fn test_detect_stack_node() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let stack_registry = Arc::new(StackRegistry::with_defaults());
        let repo_path = PathBuf::from(".");

        let stack = try_detect_stack(
            service.language,
            service.build_system,
            &service.path,
            &service.manifest,
            &repo_path,
            &stack_registry,
        )
        .unwrap();

        assert_eq!(stack.language, LanguageId::JavaScript);
        assert_eq!(stack.build_system, crate::stack::BuildSystemId::Npm);
        assert_eq!(stack.runtime, RuntimeId::Node);
        assert_eq!(stack.framework, None);
    }

    #[test]
    fn test_detect_stack_with_framework() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let package_json_content = r#"{
            "name": "test-app",
            "dependencies": {
                "express": "^4.18.0"
            }
        }"#;

        fs::write(temp_dir.path().join("package.json"), package_json_content).unwrap();

        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let stack_registry = Arc::new(StackRegistry::with_defaults());

        let stack = try_detect_stack(
            service.language,
            service.build_system,
            &service.path,
            &service.manifest,
            temp_dir.path(),
            &stack_registry,
        )
        .unwrap();

        assert_eq!(stack.language, LanguageId::JavaScript);
        assert_eq!(stack.runtime, RuntimeId::Node);
        assert_eq!(stack.framework, Some(FrameworkId::Express));
    }
}

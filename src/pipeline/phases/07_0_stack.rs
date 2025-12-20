use super::dependencies::DependencyResult;
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
            context.dependencies()?,
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
    dependencies: &DependencyResult,
    stack_registry: &Arc<StackRegistry>,
) -> Option<Stack> {
    let language_def = stack_registry.get_language(language)?;
    let runtime_name = language_def.runtime_name()?;
    let runtime = RuntimeId::from_name(runtime_name)?;

    let framework = detect_framework(service_path, dependencies, stack_registry);

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
    dependencies: &DependencyResult,
    stack_registry: &Arc<StackRegistry>,
) -> Option<FrameworkId> {
    let service_deps = dependencies.dependencies.get(service_path)?;

    for fw_id in FrameworkId::all_variants() {
        if let Some(fw) = stack_registry.get_framework(*fw_id) {
            let patterns = fw.dependency_patterns();
            for pattern in &patterns {
                if service_deps.external_deps.iter().any(|d| pattern.matches(d))
                    || service_deps.internal_deps.iter().any(|d| pattern.matches(d))
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
    use crate::pipeline::phases::structure::Service;
    use crate::stack::language::Dependency;
    use std::collections::HashMap;

    #[test]
    fn test_detect_stack_rust() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        let dependencies = DependencyResult {
            dependencies: HashMap::new(),
        };
        let stack_registry = Arc::new(StackRegistry::with_defaults());

        let stack = try_detect_stack(
            service.language,
            service.build_system,
            &service.path,
            &dependencies,
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

        let dependencies = DependencyResult {
            dependencies: HashMap::new(),
        };
        let stack_registry = Arc::new(StackRegistry::with_defaults());

        let stack = try_detect_stack(
            service.language,
            service.build_system,
            &service.path,
            &dependencies,
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
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let mut deps_info = crate::stack::language::DependencyInfo::empty();
        deps_info.external_deps.push(Dependency {
            name: "express".to_string(),
            version: Some("4.18.0".to_string()),
            is_internal: false,
        });

        let mut deps_map = HashMap::new();
        deps_map.insert(PathBuf::from("."), deps_info);

        let dependencies = DependencyResult {
            dependencies: deps_map,
        };
        let stack_registry = Arc::new(StackRegistry::with_defaults());

        let stack = try_detect_stack(
            service.language,
            service.build_system,
            &service.path,
            &dependencies,
            &stack_registry,
        )
        .unwrap();

        assert_eq!(stack.language, LanguageId::JavaScript);
        assert_eq!(stack.runtime, RuntimeId::Node);
        assert_eq!(stack.framework, Some(FrameworkId::Express));
    }
}

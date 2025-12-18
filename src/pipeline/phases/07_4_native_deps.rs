use super::scan::ScanResult;
use super::structure::Service;
use crate::pipeline::Confidence;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeDepsInfo {
    pub needs_build_deps: bool,
    pub has_native_modules: bool,
    pub has_prisma: bool,
    pub native_deps: Vec<String>,
    pub confidence: Confidence,
}

fn build_prompt(service: &Service, dependencies: &[String]) -> String {
    format!(
        r#"Detect native dependencies and build requirements for this service.

Service path: {}
Build system: {}
Language: {}

Dependencies excerpt (first 30):
{}

Respond with JSON:
{{
  "needs_build_deps": true | false,
  "has_native_modules": true | false,
  "has_prisma": true | false,
  "native_deps": ["gcc", "make", "python3"] | [],
  "confidence": "high" | "medium" | "low"
}}

Rules:
- needs_build_deps: Requires C/C++ compilers or build tools
- has_native_modules: Has native Node.js addons or Python C extensions
- has_prisma: Uses Prisma ORM (needs openssl)
- native_deps: System packages needed (apt/apk package names)
"#,
        service.path.display(),
        service.build_system.name(),
        service.language.name(),
        dependencies
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn try_deterministic(dependencies: &[String]) -> Option<NativeDepsInfo> {
    let has_prisma = dependencies.iter().any(|d| d.contains("prisma"));
    let has_native = dependencies.iter().any(|d| {
        d.contains("node-gyp")
            || d.contains("bcrypt")
            || d.contains("sharp")
            || d.contains("canvas")
            || d.contains("sqlite3")
    });

    if has_prisma || has_native {
        let mut native_deps = vec!["ca-certificates".to_string()];

        if has_prisma {
            native_deps.push("openssl".to_string());
        }

        if has_native {
            native_deps.extend(vec![
                "gcc".to_string(),
                "g++".to_string(),
                "make".to_string(),
                "python3".to_string(),
            ]);
        }

        return Some(NativeDepsInfo {
            needs_build_deps: has_native,
            has_native_modules: has_native,
            has_prisma,
            native_deps,
            confidence: Confidence::High,
        });
    }

    None
}

fn extract_dependencies(scan: &ScanResult, service: &Service) -> Result<Vec<String>> {
    // service.path is relative to repo_path
    let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

    tracing::debug!(
        "Trying to read manifest at: {} (repo: {}, service: {}, manifest: {})",
        manifest_path.display(),
        scan.repo_path.display(),
        service.path.display(),
        service.manifest
    );

    if !manifest_path.exists() {
        tracing::warn!(
            "Manifest not found at {}, returning empty dependencies",
            manifest_path.display()
        );
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    parse_dependencies(&content, &service.manifest)
}

fn parse_dependencies(content: &str, manifest: &str) -> Result<Vec<String>> {
    let mut deps = Vec::new();

    if manifest == "package.json" {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(dependencies) = json.get("dependencies").and_then(|v| v.as_object()) {
                deps.extend(dependencies.keys().map(|s| s.to_string()));
            }
            if let Some(dev_dependencies) = json.get("devDependencies").and_then(|v| v.as_object())
            {
                deps.extend(dev_dependencies.keys().map(|s| s.to_string()));
            }
        }
    }

    if manifest == "Cargo.toml" {
        if let Ok(toml) = toml::from_str::<toml::Value>(content) {
            if let Some(dependencies) = toml.get("dependencies").and_then(|v| v.as_table()) {
                deps.extend(dependencies.keys().map(|s| s.to_string()));
            }
        }
    }

    Ok(deps)
}

use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct NativeDepsPhase;

#[async_trait]
impl ServicePhase for NativeDepsPhase {
    type Output = NativeDepsInfo;

    async fn execute(&self, context: &ServiceContext<'_>) -> Result<NativeDepsInfo> {
        let dependencies =
            extract_dependencies(context.scan(), context.service).with_context(|| {
                format!(
                    "Failed to extract dependencies for service at {}",
                    context.service.path.display()
                )
            })?;

        let result = if let Some(deterministic) = try_deterministic(&dependencies) {
            deterministic
        } else {
            let prompt = build_prompt(context.service, &dependencies);
            super::llm_helper::query_llm_with_logging(
                context.llm_client(),
                prompt,
                400,
                "native_deps",
                context.heuristic_logger(),
            )
            .await?
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_prisma() {
        let deps = vec!["express".to_string(), "@prisma/client".to_string()];
        let result = try_deterministic(&deps).unwrap();

        assert!(result.has_prisma);
        assert!(result.native_deps.contains(&"openssl".to_string()));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_deterministic_native_modules() {
        let deps = vec!["express".to_string(), "bcrypt".to_string()];
        let result = try_deterministic(&deps).unwrap();

        assert!(result.has_native_modules);
        assert!(result.needs_build_deps);
        assert!(result.native_deps.contains(&"gcc".to_string()));
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: std::path::PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let deps = vec!["express".to_string(), "bcrypt".to_string()];
        let prompt = build_prompt(&service, &deps);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("bcrypt"));
    }
}

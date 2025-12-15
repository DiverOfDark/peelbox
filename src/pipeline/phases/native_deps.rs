use super::scan::ScanResult;
use super::structure::Service;
use crate::llm::LLMClient;
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
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
        service.build_system,
        service.language,
        dependencies.iter().take(30).cloned().collect::<Vec<_>>().join(", ")
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
) -> Result<NativeDepsInfo> {
    let dependencies = extract_dependencies(scan, service)?;

    if let Some(deterministic) = try_deterministic(&dependencies) {
        return Ok(deterministic);
    }

    let prompt = build_prompt(service, &dependencies);
    super::llm_helper::query_llm(llm_client, prompt, 400, "native deps detection").await
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
    let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

    if !manifest_path.exists() {
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    let mut deps = Vec::new();

    if service.manifest == "package.json" {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(dependencies) = json.get("dependencies").and_then(|v| v.as_object()) {
                deps.extend(dependencies.keys().cloned());
            }
            if let Some(dev_dependencies) =
                json.get("devDependencies").and_then(|v| v.as_object())
            {
                deps.extend(dev_dependencies.keys().cloned());
            }
        }
    }

    if service.manifest == "Cargo.toml" {
        if let Ok(toml) = toml::from_str::<toml::Value>(&content) {
            if let Some(dependencies) = toml.get("dependencies").and_then(|v| v.as_table()) {
                deps.extend(dependencies.keys().cloned());
            }
        }
    }

    Ok(deps)
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
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let deps = vec!["express".to_string(), "bcrypt".to_string()];
        let prompt = build_prompt(&service, &deps);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("bcrypt"));
    }
}

use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

pub struct PackageJsonParser;

impl ManifestParser for PackageJsonParser {
    fn filenames(&self) -> &[&str] {
        &["package.json"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;

        let name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_start = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .is_some();

        let build_system = match json.get("packageManager").and_then(|v| v.as_str()) {
            Some(pm) if pm.starts_with("yarn") => BuildSystemId::Yarn,
            Some(pm) if pm.starts_with("pnpm") => BuildSystemId::Pnpm,
            Some(pm) if pm.starts_with("bun") => BuildSystemId::Bun,
            _ => BuildSystemId::Npm,
        };

        let pkg_manager = match &build_system {
            BuildSystemId::Yarn => "yarn",
            BuildSystemId::Pnpm => "pnpm",
            BuildSystemId::Bun => "bun",
            _ => "npm",
        };

        let workspace = json.get("workspaces").and_then(|ws| {
            let members = match ws {
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                serde_json::Value::Object(obj) => obj
                    .get("packages")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => return None,
            };
            Some(Workspace {
                members,
                orchestrator: None,
            })
        });

        let dependencies = super::parse_npm_deps(&json);

        // Only detect TypeScript if it's a runtime dependency (not devDependency)
        let language = if dependencies
            .iter()
            .any(|d| d.name == "typescript" && d.scope == DepScope::Runtime)
        {
            LanguageId::TypeScript
        } else {
            LanguageId::JavaScript
        };

        let start_script = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .and_then(|v| v.as_str());

        let entrypoint = if let Some(script) = start_script {
            // scripts.start takes priority — it defines how to run the application
            if script.starts_with("node ") {
                Some(script.to_string())
            } else {
                Some(format!("{} start", pkg_manager))
            }
        } else {
            // No scripts.start → no entrypoint (main field is for library entry, not runtime)
            None
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system,
            runtime: RuntimeId::Node,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec!["nodejs".into(), pkg_manager.into(), "ca-certificates".into()],
                commands: vec![
                    "npm ci".to_string(),
                    format!("{} run build", pkg_manager),
                ],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        "npm ci".to_string(),
                        format!("cd {{module}} && {} run build", pkg_manager),
                    ],
                    member_artifacts: None,
                }),
                env: BTreeMap::new(),
                cache_dirs: vec![".npm".into(), "node_modules".into()],
                artifacts: vec![(".".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "nodejs".into(),
                    "npm".into(),
                    "busybox".into(),
                    "dumb-init".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PackageJsonParser))
}

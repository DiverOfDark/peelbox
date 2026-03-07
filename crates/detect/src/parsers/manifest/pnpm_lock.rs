use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const JAVASCRIPT: LanguageId = LanguageId::new("javascript");
const PNPM: BuildSystemId = BuildSystemId::new("pnpm");
const NODE: RuntimeId = RuntimeId::new("node");

/// Detects pnpm projects by presence of pnpm-lock.yaml.
pub struct PnpmLockParser;

impl ManifestParser for PnpmLockParser {
    fn filenames(&self) -> &[&str] {
        &["pnpm-lock.yaml"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        let dir = path.parent()?;
        let pkg_json_path = dir.join("package.json");
        let abs_pkg_json = if pkg_json_path.is_absolute() {
            pkg_json_path
        } else {
            return None;
        };
        let pkg_content = std::fs::read_to_string(&abs_pkg_json).ok()?;
        let json: serde_json::Value = serde_json::from_str(&pkg_content).ok()?;

        let name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_start = json.get("scripts").and_then(|s| s.get("start")).is_some();

        // Lock file parsers don't carry dependencies — framework detection
        // happens on the sibling package.json manifest instead.
        let dependencies: Vec<Dependency> = Vec::new();

        let language = JAVASCRIPT;

        let start_script = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .and_then(|v| v.as_str());

        let entrypoint = if let Some(script) = start_script {
            if script.starts_with("node ") {
                Some(script.to_string())
            } else {
                Some("pnpm start".to_string())
            }
        } else {
            json.get("main")
                .and_then(|v| v.as_str())
                .map(|m| format!("node {}", m))
        };

        // Extract Node.js version from engines.node or volta.node
        let node_pkg = json
            .get("engines")
            .and_then(|e| e.get("node"))
            .and_then(|v| v.as_str())
            .and_then(super::package_json::extract_node_major)
            .or_else(|| {
                json.get("volta")
                    .and_then(|v| v.get("node"))
                    .and_then(|v| v.as_str())
                    .and_then(super::package_json::extract_node_major)
            })
            .map(|v| format!("nodejs-{}", v))
            .unwrap_or_else(|| "nodejs".into());

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: PNPM,
            runtime: NODE,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    node_pkg.clone(),
                    "pnpm".into(),
                    "build-base".into(),
                    "python".into(),
                    "npm".into(),
                    "ca-certificates".into(),
                ],
                commands: {
                    let has_build = json.get("scripts").and_then(|s| s.get("build")).is_some();
                    let mut cmds = vec!["pnpm install".into()];
                    if has_build {
                        cmds.push("pnpm build".into());
                    }
                    cmds
                },
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".pnpm-store".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    node_pkg,
                    "pnpm".into(),
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
    crate::registry::ManifestParserEntry(|| Box::new(PnpmLockParser))
}

// ── Build System Profile ────────────────────────────────────────────────────

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        merge_priority: true,
        non_root_entrypoint_override: Some(&["pnpm", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(PNPM)
    })
}

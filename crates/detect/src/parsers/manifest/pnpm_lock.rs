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

        let entrypoint = json
            .get("main")
            .and_then(|v| v.as_str())
            .map(|m| format!("node {}", m));

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
                    "nodejs".into(),
                    "pnpm".into(),
                    "build-base".into(),
                    "python".into(),
                    "npm".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["pnpm install".into(), "pnpm build".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".pnpm-store".into(), "node_modules".into()],
                artifacts: vec![(".".into(), "/app".into())],
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
    crate::registry::ManifestParserEntry(|| Box::new(PnpmLockParser))
}

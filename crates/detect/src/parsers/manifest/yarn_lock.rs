use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const JAVASCRIPT: LanguageId = LanguageId::new("javascript");
const YARN: BuildSystemId = BuildSystemId::new("yarn");
const NODE: RuntimeId = RuntimeId::new("node");

/// Detects Yarn projects by presence of yarn.lock.
/// Reads the sibling package.json for project info but overrides build system to Yarn.
pub struct YarnLockParser;

impl ManifestParser for YarnLockParser {
    fn filenames(&self) -> &[&str] {
        &["yarn.lock"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        // Read the sibling package.json
        let dir = path.parent()?;
        let pkg_json_path = dir.join("package.json");
        // Use the relative path for the sibling
        let abs_pkg_json = if pkg_json_path.is_absolute() {
            pkg_json_path
        } else {
            // During pipeline, the path is relative, we need to find it
            // The PackageJsonParser will be called separately; this parser
            // needs to read from absolute path resolved during tree walk.
            // We return None if we can't find it.
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

        // Detect build script
        let build_script = json
            .get("scripts")
            .and_then(|s| s.get("build"))
            .and_then(|v| v.as_str());

        let has_tsc = build_script.map(|s| s.contains("tsc")).unwrap_or(false);

        let build_cmd = if has_tsc {
            Some("./node_modules/.bin/tsc".to_string())
        } else if build_script.is_some() {
            Some("yarn run build".to_string())
        } else {
            None
        };

        // Check if packageManager specifies Yarn >= 2 (needs corepack)
        let needs_corepack = json
            .get("packageManager")
            .and_then(|v| v.as_str())
            .filter(|pm| pm.starts_with("yarn@"))
            .and_then(|pm| pm.strip_prefix("yarn@"))
            .and_then(|ver| ver.split('.').next())
            .and_then(|major| major.parse::<u32>().ok())
            .is_some_and(|major| major >= 2);

        let mut commands = Vec::new();
        if needs_corepack {
            commands.push("corepack enable".to_string());
            // Yarn >= 2 (Berry) does not support --network-timeout/--network-concurrency
            commands.push("yarn install".into());
        } else {
            commands.push("yarn install --network-timeout 100000 --network-concurrency 1".into());
        }
        if let Some(cmd) = build_cmd {
            commands.push(cmd);
        }

        let mut build_packages = vec!["nodejs".into(), "yarn".into(), "ca-certificates".into()];
        if needs_corepack {
            build_packages.push("corepack".into());
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: YARN,
            runtime: NODE,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: build_packages,
                commands,
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".yarn-cache".into()],
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
    crate::registry::ManifestParserEntry(|| Box::new(YarnLockParser))
}

// ── Build System Profile ────────────────────────────────────────────────────

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        merge_priority: true,
        non_root_entrypoint_override: Some(&["yarn", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(YARN)
    })
}

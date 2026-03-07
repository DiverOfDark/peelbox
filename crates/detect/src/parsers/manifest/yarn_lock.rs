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

        let start_script = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .and_then(|v| v.as_str());

        let entrypoint = if start_script.is_some() {
            // Always use `yarn start` to ensure Yarn PnP resolution works
            Some("yarn start".to_string())
        } else {
            json.get("main")
                .and_then(|v| v.as_str())
                .map(|m| format!("node {}", m))
        };

        // Detect build script
        let build_script = json
            .get("scripts")
            .and_then(|s| s.get("build"))
            .and_then(|v| v.as_str());

        let build_cmd = if build_script.is_some() {
            Some("yarn run build".to_string())
        } else {
            None
        };

        // Detect Yarn Berry (>= 2) via multiple signals:
        // 1. packageManager field in package.json (e.g., "yarn@4.9.2")
        // 2. __metadata: header in yarn.lock (Berry lock format)
        // 3. yarnPath in sibling .yarnrc.yml (bundled Berry binary)
        let has_package_manager_berry = json
            .get("packageManager")
            .and_then(|v| v.as_str())
            .filter(|pm| pm.starts_with("yarn@"))
            .and_then(|pm| pm.strip_prefix("yarn@"))
            .and_then(|ver| ver.split('.').next())
            .and_then(|major| major.parse::<u32>().ok())
            .is_some_and(|major| major >= 2);
        let has_berry_lockfile = _content.contains("__metadata:");
        let has_yarnrc_path = dir.join(".yarnrc.yml").exists();
        let is_berry = has_package_manager_berry || has_berry_lockfile || has_yarnrc_path;
        // corepack is only needed when packageManager is set (corepack reads it)
        // For yarnPath-based Berry, the bundled binary is used directly
        let needs_corepack = has_package_manager_berry;

        let mut commands = Vec::new();
        if needs_corepack {
            commands.push("corepack enable".to_string());
        }
        if is_berry {
            // Yarn >= 2 (Berry) does not support --network-timeout/--network-concurrency
            commands.push("yarn install".into());
        } else {
            commands.push("yarn install --network-timeout 100000 --network-concurrency 1".into());
        }
        if let Some(cmd) = build_cmd {
            commands.push(cmd);
        }

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

        let mut build_packages = vec![node_pkg.clone(), "yarn".into(), "ca-certificates".into()];
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
                env: {
                    let mut env = BTreeMap::new();
                    if is_berry {
                        // Force project-local cache so packages are copied with artifacts
                        env.insert("YARN_ENABLE_GLOBAL_CACHE".into(), "false".into());
                    }
                    env
                },
                cache_dirs: vec![".yarn-cache".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: {
                    let mut pkgs = vec![
                        node_pkg,
                        "yarn".into(),
                        "busybox".into(),
                        "dumb-init".into(),
                        "ca-certificates".into(),
                    ];
                    if needs_corepack {
                        pkgs.push("corepack".into());
                    }
                    pkgs
                },
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

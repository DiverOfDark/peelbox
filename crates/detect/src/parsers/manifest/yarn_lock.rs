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

        // Entrypoint will be adjusted later (after Berry detection) if needed
        let base_entrypoint = if start_script.is_some() {
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
        // corepack is only needed when packageManager is set (corepack reads it).
        // For yarnPath-based Berry, the bundled binary is used directly.
        // Additionally, corepack requires Node >= 20 (uses URL.canParse internally),
        // so we skip it for older Node versions to avoid runtime errors.
        let node_major_for_corepack = json
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
            .and_then(|v| v.parse::<u32>().ok());
        let corepack_compatible = node_major_for_corepack
            .map(|v| v >= 20)
            .unwrap_or(true); // Default to compatible if no version specified
        let needs_corepack = has_package_manager_berry && corepack_compatible;

        // Extract Yarn version from packageManager field (e.g., "yarn@3.2.4")
        let yarn_version = json
            .get("packageManager")
            .and_then(|v| v.as_str())
            .and_then(|pm| pm.strip_prefix("yarn@"))
            .map(|v| v.split('+').next().unwrap_or(v).to_string());

        // Read yarnPath from .yarnrc.yml (e.g., ".yarn/releases/yarn-3.2.4.cjs")
        let yarn_path = dir
            .join(".yarnrc.yml")
            .is_file()
            .then(|| std::fs::read_to_string(dir.join(".yarnrc.yml")).ok())
            .flatten()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("yarnPath:"))
                    .map(|l| l.trim_start_matches("yarnPath:").trim().trim_matches('"').to_string())
            });

        // Berry without corepack: use the bundled yarn binary (yarnPath) directly.
        // This is only needed when packageManager explicitly requests Berry but corepack
        // can't be used (e.g., Node < 20). When Berry is detected only via .yarnrc.yml
        // or lockfile, the globally installed Yarn Classic v1 can delegate to Berry
        // via .yarnrc.yml's yarnPath setting.
        let use_bundled_berry =
            is_berry && has_package_manager_berry && !corepack_compatible && yarn_path.is_some();

        let mut commands = Vec::new();
        if needs_corepack {
            commands.push("corepack enable".to_string());
        } else if is_berry && has_package_manager_berry && !corepack_compatible {
            if use_bundled_berry {
                // Use the bundled Berry binary via yarnPath — no install needed.
                // Yarn Classic reads .yarnrc.yml and delegates to the bundled binary.
                // But Yarn Classic v1 does NOT read .yarnrc.yml, so we create a wrapper.
                if let Some(ref yp) = yarn_path {
                    commands.push(format!(
                        "mkdir -p /usr/local/bin && printf '#!/bin/sh\\nnode /build/{}  \"$@\"\\n' > /usr/local/bin/yarn && chmod +x /usr/local/bin/yarn",
                        yp
                    ));
                }
            } else if let Some(ref ver) = yarn_version {
                // No bundled binary; install the specific version globally via npm.
                commands.push(format!("npm install -g yarn@{}", ver));
            } else {
                commands.push("npm install -g yarn@berry".to_string());
            }
        }
        if is_berry {
            // Yarn >= 2 (Berry) does not support --network-timeout/--network-concurrency
            commands.push("yarn install".into());
        } else {
            commands.push("yarn install --network-timeout 100000 --network-concurrency 1".into());
        }
        // Build member_commands for workspace-aware builds.
        // The install command runs at the workspace root, but the build command
        // must run in the member's directory.
        let mut member_commands = commands.clone();
        if let Some(ref cmd) = build_cmd {
            commands.push(cmd.clone());
            member_commands.push(format!("cd {{module}} && {}", cmd));
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
        if use_bundled_berry {
            // npm is needed in build for `npm install -g` (no-op here since we use wrapper)
            // but add npm to build packages since build may still need it
            if !build_packages.contains(&"npm".to_string()) {
                build_packages.push("npm".into());
            }
        }

        // Compute the final entrypoint for Berry without corepack:
        // Use the bundled Berry binary directly via `node .yarn/releases/yarn-X.cjs start`
        let entrypoint = if use_bundled_berry {
            if let Some(ref yp) = yarn_path {
                base_entrypoint.map(|cmd| {
                    // Replace "yarn" with "node <yarnPath>" in the entrypoint
                    if cmd.starts_with("yarn") {
                        let rest = cmd.strip_prefix("yarn").unwrap_or("");
                        format!("node {}{}", yp, rest)
                    } else {
                        cmd
                    }
                })
            } else {
                base_entrypoint
            }
        } else {
            base_entrypoint
        };

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
                member_transform: Some(MemberBuildTransform {
                    member_commands,
                    member_artifacts: None,
                }),
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
                            build_image: None,
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

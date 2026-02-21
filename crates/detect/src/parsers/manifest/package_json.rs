use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const JAVASCRIPT: LanguageId = LanguageId::new("javascript");
const TYPESCRIPT: LanguageId = LanguageId::new("typescript");
const NPM: BuildSystemId = BuildSystemId::new("npm");
const YARN: BuildSystemId = BuildSystemId::new("yarn");
const PNPM: BuildSystemId = BuildSystemId::new("pnpm");
const BUN: BuildSystemId = BuildSystemId::new("bun");
const NODE: RuntimeId = RuntimeId::new("node");

inventory::submit! {
    LanguageMeta { slug: "javascript", display_name: "JavaScript", aliases: &[] }
}
inventory::submit! {
    LanguageMeta { slug: "typescript", display_name: "TypeScript", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "npm", display_name: "npm", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "yarn", display_name: "Yarn", aliases: &["yarn"] }
}
inventory::submit! {
    BuildSystemMeta { slug: "pnpm", display_name: "pnpm", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "bun", display_name: "Bun", aliases: &["bun"] }
}
inventory::submit! {
    RuntimeMeta { slug: "node", display_name: "Node", aliases: &["node"] }
}

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
        let has_start = json.get("scripts").and_then(|s| s.get("start")).is_some();
        let has_build = json.get("scripts").and_then(|s| s.get("build")).is_some();

        let build_system = match json.get("packageManager").and_then(|v| v.as_str()) {
            Some(pm) if pm.starts_with("yarn") => YARN,
            Some(pm) if pm.starts_with("pnpm") => PNPM,
            Some(pm) if pm.starts_with("bun") => BUN,
            _ => NPM,
        };

        let pkg_manager = match build_system {
            YARN => "yarn",
            PNPM => "pnpm",
            BUN => "bun",
            _ => "npm",
        };

        // Extract Node.js version from engines.node or volta.node
        let node_major = json
            .get("engines")
            .and_then(|e| e.get("node"))
            .and_then(|v| v.as_str())
            .and_then(extract_node_major)
            .or_else(|| {
                json.get("volta")
                    .and_then(|v| v.get("node"))
                    .and_then(|v| v.as_str())
                    .and_then(extract_node_major)
            });

        let node_build_pkg = node_major
            .as_ref()
            .map(|v| format!("nodejs-{}", v))
            .unwrap_or_else(|| "nodejs".into());
        let node_runtime_pkg = node_build_pkg.clone();

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
            TYPESCRIPT
        } else {
            JAVASCRIPT
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

        let mut build_commands = vec!["npm ci".to_string()];
        if has_build {
            build_commands.push(format!("{} run build", pkg_manager));
        }

        let mut member_commands = vec!["npm ci".to_string()];
        if has_build {
            member_commands.push(format!("cd {{module}} && {} run build", pkg_manager));
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system,
            runtime: NODE,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec![node_build_pkg, pkg_manager.into(), "ca-certificates".into()],
                commands: build_commands,
                member_transform: Some(MemberBuildTransform {
                    member_commands,
                    member_artifacts: None,
                }),
                env: BTreeMap::new(),
                cache_dirs: vec![".npm".into()],
                artifacts: vec![(".".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    node_runtime_pkg,
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

/// Extract major version number from a Node.js version constraint.
/// Handles: ">=18", "^20.0.0", "~18.0", "20", "v18.12.0", "18.x"
fn extract_node_major(constraint: &str) -> Option<String> {
    let cleaned = constraint
        .trim()
        .trim_start_matches(['>', '<', '=', '^', '~', 'v']);
    let major = cleaned.split('.').next()?.trim_end_matches('x');
    if major.chars().all(|c| c.is_ascii_digit()) && !major.is_empty() {
        Some(major.to_string())
    } else {
        None
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PackageJsonParser))
}

// ── Build System Profiles ───────────────────────────────────────────────────

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        non_root_entrypoint_override: Some(&["npm", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(NPM)
    })
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        merge_priority: true,
        non_root_entrypoint_override: Some(&["pnpm", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(PNPM)
    })
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        merge_priority: true,
        non_root_entrypoint_override: Some(&["yarn", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(YARN)
    })
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        non_root_entrypoint_override: Some(&["bun", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(BUN)
    })
}

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
            _ => {
                // Detect Bun from lockfile when packageManager is not set
                let has_bun_lock = path.is_absolute()
                    && path
                        .parent()
                        .map(|dir| dir.join("bun.lockb").exists() || dir.join("bun.lock").exists())
                        .unwrap_or(false);
                let has_engines_bun = json.get("engines").and_then(|e| e.get("bun")).is_some();
                if has_bun_lock || has_engines_bun {
                    BUN
                } else {
                    NPM
                }
            }
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

        let main_file = json.get("main").and_then(|v| v.as_str());
        let module_file = json.get("module").and_then(|v| v.as_str());

        // Read tsconfig.json outDir if present — used to adjust the main entrypoint
        // when TypeScript compiles to a different directory. Follows `extends` chains
        // to resolve outDir from parent configs.
        let ts_out_dir = path
            .parent()
            .and_then(|dir| resolve_tsconfig_out_dir(&dir.join("tsconfig.json"), 0));

        let entrypoint = if let Some(script) = start_script {
            // scripts.start takes priority — it defines how to run the application
            if script.starts_with("node ") {
                Some(script.to_string())
            } else {
                Some(format!("{} start", pkg_manager))
            }
        } else if let Some(main) = main_file {
            // main field fallback — treat as the application entry point.
            // When TypeScript has outDir, the compiled JS is under that directory.
            let resolved_main = if let Some(ref out_dir) = ts_out_dir {
                // Only prepend outDir if main doesn't already start with it
                if main.starts_with(&format!("{}/", out_dir))
                    || main.starts_with(&format!("{out_dir}\\"))
                {
                    main.to_string()
                } else {
                    format!("{}/{}", out_dir, main)
                }
            } else {
                main.to_string()
            };
            Some(format!("node {}", resolved_main))
        } else if build_system == BUN {
            // Bun fallback: use module field or index.ts as the entry point
            if let Some(module) = module_file {
                Some(format!("bun run {}", module))
            } else {
                // Check for common entry files in the project directory
                let entry = path.parent().and_then(|dir| {
                    for candidate in &[
                        "index.ts",
                        "index.tsx",
                        "index.js",
                        "server.ts",
                        "server.js",
                    ] {
                        if dir.join(candidate).exists() {
                            return Some(*candidate);
                        }
                    }
                    None
                });
                entry.map(|e| format!("bun run {}", e))
            }
        } else {
            // No scripts.start, no main → entrypoint from framework fallback or none
            None
        };

        let install_cmd = match build_system {
            PNPM => "pnpm install".to_string(),
            YARN => "yarn install".to_string(),
            BUN => "bun install".to_string(),
            _ => {
                // Use npm ci if package-lock.json exists, npm install otherwise
                let has_lockfile = path
                    .parent()
                    .map(|d| d.join("package-lock.json").exists())
                    .unwrap_or(false);
                if has_lockfile {
                    "npm ci".to_string()
                } else {
                    "npm install".to_string()
                }
            }
        };

        // Detect TypeScript projects without a build script — add tsc compilation step
        let has_typescript_dep = json
            .get("devDependencies")
            .and_then(|d| d.get("typescript"))
            .is_some()
            || json
                .get("dependencies")
                .and_then(|d| d.get("typescript"))
                .is_some();
        let has_tsconfig = path.is_absolute()
            && path
                .parent()
                .map(|d| d.join("tsconfig.json").exists())
                .unwrap_or(false);

        // Check for frameworks that require specific build commands even without scripts.build
        let has_next = json
            .get("dependencies")
            .and_then(|d| d.get("next"))
            .is_some();

        let mut build_commands = vec![install_cmd.clone()];
        if has_build {
            build_commands.push(format!("{} run build", pkg_manager));
        } else if has_next {
            // Next.js requires `next build` — `npx tsc` is insufficient
            build_commands.push("npx next build".to_string());
        } else if has_typescript_dep && has_tsconfig {
            build_commands.push("npx tsc".to_string());
        }

        let mut member_commands = vec![install_cmd];
        if has_build {
            member_commands.push(format!("cd {{module}} && {} run build", pkg_manager));
        } else if has_next {
            member_commands.push("cd {module} && npx next build".to_string());
        } else if has_typescript_dep && has_tsconfig {
            member_commands.push("cd {module} && npx tsc".to_string());
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
                setup_commands: vec![],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    node_runtime_pkg,
                    pkg_manager.into(),
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

/// Resolves `outDir` from a tsconfig.json file, following `extends` chains up to 5 levels deep.
/// TypeScript's `extends` merges compilerOptions from parent configs, with child values taking
/// precedence. We walk the chain to find the effective outDir.
fn resolve_tsconfig_out_dir(tsconfig_path: &std::path::Path, depth: u8) -> Option<String> {
    if depth > 5 {
        return None;
    }
    let content = std::fs::read_to_string(tsconfig_path).ok()?;
    let tsconfig: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Check this config's own outDir first (child takes precedence)
    let local_out_dir = tsconfig
        .get("compilerOptions")
        .and_then(|co| co.get("outDir"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim_end_matches('/').to_string());

    if local_out_dir.is_some() {
        return local_out_dir;
    }

    // Follow extends chain to find outDir in parent config
    let extends = tsconfig.get("extends").and_then(|v| v.as_str())?;
    let parent_dir = tsconfig_path.parent()?;
    let mut extended_path = parent_dir.join(extends);
    // If the extends path doesn't have a .json extension, try adding it
    if !extended_path.exists() && extended_path.extension().is_none() {
        extended_path.set_extension("json");
    }
    resolve_tsconfig_out_dir(&extended_path, depth + 1)
}

/// Extract major version number from a Node.js version constraint.
/// Handles: ">=18", "^20.0.0", "~18.0", "20", "v18.12.0", "18.x"
pub(crate) fn extract_node_major(constraint: &str) -> Option<String> {
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
        non_root_entrypoint_override: Some(&["bun", "start"]),
        adjusts_workspace_member_workdir: true,
        ..BuildSystemConfig::new(BUN)
    })
}

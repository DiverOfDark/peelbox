use crate::helpers::extract_project_dir;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_core::output::schema::UniversalBuild;
use peelbox_wolfi::WolfiPackageIndex;
use std::collections::BTreeMap;
use std::path::Path;
use tracing::debug;

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

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["JavaScript", "TypeScript"],
        extensions: &["js", "ts", "mjs", "cjs"],
        port_patterns: &[
            r"\.listen\(\s*(\d{4,5})",
            r#"port["\s:=]+(\d{4,5})"#,
            r"\|\|\s*(\d{4,5})",
        ],
        health_patterns: &[r#"app\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
        env_var_patterns: &[r"process\.env\.([A-Z_][A-Z0-9_]*)"],
    }
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

// ── Node.js version detection and resolution ────────────────────────────

/// Read Node.js version from `.nvmrc` or `.node-version` file.
/// Returns the major version string (e.g., "18", "20").
pub(crate) fn read_node_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    for dir in &[project_dir, repo_root] {
        for filename in &[".nvmrc", ".node-version"] {
            let path = dir.join(filename);
            if let Ok(content) = std::fs::read_to_string(&path) {
                let trimmed = content.trim();
                if let Some(version) = parse_node_version_string(trimmed) {
                    return Some(version);
                }
            }
        }
    }
    None
}

/// Parse a Node.js version string: strip 'v' prefix, map LTS codenames, extract major.
pub(crate) fn parse_node_version_string(s: &str) -> Option<String> {
    let s = s.trim().trim_start_matches('v');

    // Map LTS codenames
    let s = match s.to_lowercase().as_str() {
        "lts/iron" => "20",
        "lts/hydrogen" => "18",
        "lts/gallium" => "16",
        "lts/fermium" => "14",
        "lts/*" => return None, // Can't resolve "latest LTS" statically
        _ => s,
    };

    // Extract major version
    let major = s.split('.').next()?;
    if major.chars().all(|c| c.is_ascii_digit()) && !major.is_empty() {
        Some(major.to_string())
    } else {
        None
    }
}

/// The minimum major Node.js version available in Wolfi.
/// Versions below this threshold require direct download.
pub(crate) const MIN_WOLFI_NODE_MAJOR: u32 = 16;

/// When a pinned Node.js version (e.g., `nodejs-14`) is not available in Wolfi,
/// switch to installing it via `n` (node version manager) which downloads from nodejs.org.
pub(crate) fn resolve_node_version(build: &mut UniversalBuild, wolfi: &WolfiPackageIndex) {
    if !matches!(
        build.metadata.language.as_str(),
        "JavaScript" | "TypeScript"
    ) {
        return;
    }

    // Find a nodejs-X package that doesn't exist in Wolfi
    let pinned_version = build.build.packages.iter().find_map(|pkg| {
        pkg.strip_prefix("nodejs-").and_then(|version_str| {
            let major: u32 = version_str.parse().ok()?;
            if major < MIN_WOLFI_NODE_MAJOR && !wolfi.has_package(pkg) {
                Some(version_str.to_string())
            } else {
                None
            }
        })
    });

    let Some(version) = pinned_version else {
        return;
    };

    debug!(
        version = %version,
        "Pinned Node.js version not in Wolfi, switching to n (node version manager)"
    );

    // Replace the unavailable nodejs-X package with curl (for downloading n)
    for pkg in build.build.packages.iter_mut() {
        if pkg.starts_with("nodejs-") && !wolfi.has_package(pkg.as_str()) {
            *pkg = "curl".to_string();
            break;
        }
    }

    // Ensure required packages are present
    for required_pkg in &["ca-certificates", "build-base", "bash", "libstdc++"] {
        if !build.build.packages.contains(&required_pkg.to_string()) {
            build.build.packages.push(required_pkg.to_string());
        }
    }

    // Remove npm from build packages (it comes bundled with the Node.js install)
    build.build.packages.retain(|p| p != "npm");

    // Also remove npm-dependent commands (global installs, symlinks) since npm
    // won't be available until after the `n` installer runs in build commands.
    build
        .build
        .commands
        .retain(|c: &String| !c.contains("npm install -g") && !c.contains("node_modules/npm"));

    // Prepend Node.js installation command using `n` (node version manager)
    let install_cmd = format!(
        "mkdir -p /usr/local/bin && curl -fsSL https://raw.githubusercontent.com/tj/n/master/bin/n -o /usr/local/bin/n && chmod +x /usr/local/bin/n && n {}",
        version
    );
    build.build.commands.insert(0, install_cmd);

    // Set PATH to include the installed Node.js
    build.build.env.insert(
        "PATH".to_string(),
        "/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
    );

    // Fix runtime packages too
    let mut needs_runtime_fix = false;
    for pkg in build.runtime.packages.iter_mut() {
        if pkg.starts_with("nodejs-") && !wolfi.has_package(pkg.as_str()) {
            *pkg = "curl".to_string();
            needs_runtime_fix = true;
            break;
        }
    }

    if needs_runtime_fix {
        for required_pkg in &["ca-certificates", "bash", "libstdc++"] {
            if !build.runtime.packages.contains(&required_pkg.to_string()) {
                build.runtime.packages.push(required_pkg.to_string());
            }
        }
        build.runtime.packages.retain(|p| p != "npm");

        let original_cmd = build.runtime.command.clone();
        if !original_cmd.is_empty() {
            let install_cmd = format!(
                "mkdir -p /usr/local/bin && curl -fsSL https://raw.githubusercontent.com/tj/n/master/bin/n -o /usr/local/bin/n && chmod +x /usr/local/bin/n && n {} > /dev/null 2>&1",
                version
            );
            let original_cmd_str = original_cmd.join(" ");
            build.runtime.command = vec![
                "sh".to_string(),
                "-c".to_string(),
                format!("{} && {}", install_cmd, original_cmd_str),
            ];
        }

        build.runtime.env.insert(
            "PATH".to_string(),
            "/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        );
    }
}

/// Wolfi ships only the latest `pnpm` / `npm` / `yarn` packages, and those track
/// upstream: pnpm 11 requires Node >= 22.13, npm 11 requires Node >= 20.17. When
/// a project pins an older Node major (via `engines.node`, `.nvmrc`, mise, etc.)
/// the latest package manager crashes before it can even run (e.g. pnpm using the
/// `node:sqlite` builtin, or npm refusing the Node version).
///
/// A project's declared Node version is a *minimum* it supports, not a ceiling —
/// running on a newer Node is compatible. So when the pinned major is below what
/// the package-manager tooling needs, floor it up to that minimum. pnpm's own
/// version self-management (the `packageManager` field) still installs the exact
/// pinned pnpm once a compatible Node is in place.
pub(crate) fn ensure_node_tooling_floor(build: &mut UniversalBuild) {
    if !matches!(
        build.metadata.language.as_str(),
        "JavaScript" | "TypeScript"
    ) {
        return;
    }

    // pnpm 11 needs Node >= 22.13 (it uses the `node:sqlite` builtin). npm 11
    // only hard-rejects Node 16 and older (Node 18 still works with a warning),
    // so a floor of 18 is enough to keep npm/yarn builds running without churning
    // fixtures that already pin a supported version.
    let min_major: u32 = match build.metadata.build_system.to_ascii_lowercase().as_str() {
        "pnpm" => 22,
        "npm" => 18,
        // Classic yarn (1.x) strictly enforces `engines.node` and runs fine on old
        // Node, so bumping it would make `yarn install` reject the build. Only bump
        // when the build also pulls in the Wolfi `npm` package (e.g. Yarn Berry via
        // corepack, or a node-gyp step), which is what actually rejects Node < 18.
        "yarn" if build.build.packages.iter().any(|p| p == "npm") => 18,
        _ => return, // classic yarn without npm, bun (bundles its own runtime), etc.
    };

    fn floor_node(pkgs: &mut [String], min_major: u32) {
        for pkg in pkgs.iter_mut() {
            if let Some(version_str) = pkg.strip_prefix("nodejs-") {
                if let Ok(major) = version_str.parse::<u32>() {
                    // Only adjust versions that Wolfi actually ships (and that
                    // therefore use the Wolfi pnpm/npm packages). Older pins go
                    // through the `n` installer with Node's own bundled npm, which
                    // doesn't have the latest-tooling compatibility problem.
                    if major >= MIN_WOLFI_NODE_MAJOR && major < min_major {
                        *pkg = format!("nodejs-{}", min_major);
                    }
                }
            }
        }
    }

    floor_node(&mut build.build.packages, min_major);
    floor_node(&mut build.runtime.packages, min_major);
}

/// pnpm 10.16+/11 refuses to run dependency lifecycle scripts (esbuild's
/// `postinstall`, sharp's `install`, native `node-gyp` builds, etc.) unless they
/// are explicitly approved, and `pnpm install` then exits non-zero
/// (`ERR_PNPM_IGNORED_BUILDS`). Prepend a global config command that approves all
/// builds so those steps run during the image build.
///
/// Applied as a post-processing pass (not in the parser) because most pnpm
/// projects are detected from `pnpm-lock.yaml`, whose parser produces the build
/// commands.
pub(crate) fn ensure_pnpm_allow_builds(build: &mut UniversalBuild) {
    if build.metadata.build_system.as_str() != "pnpm" {
        return;
    }

    const ALLOW_BUILDS: &str = "pnpm config set dangerously-allow-all-builds true";

    let prepend_if_needed = |commands: &mut Vec<String>| {
        let uses_pnpm = commands.iter().any(|c| c.contains("pnpm "));
        let already = commands
            .iter()
            .any(|c| c.contains("dangerously-allow-all-builds"));
        if uses_pnpm && !already {
            commands.insert(0, ALLOW_BUILDS.to_string());
        }
    };

    prepend_if_needed(&mut build.build.commands);
}

// ── Node.js native dependency detection ──────────────────────────────────

/// Wolfi's `npm` package (11.x) does not bundle `node-gyp`. npm internally
/// calls `require.resolve('node-gyp/bin/node-gyp.js')` for every lifecycle
/// script, so any `npm ci` / `npm install` will fail when a dependency has
/// install hooks (esbuild, prisma, etc.).
pub(crate) fn ensure_npm_node_gyp(build: &mut UniversalBuild) {
    if !build.build.packages.iter().any(|p| p == "npm") {
        return;
    }

    let gyp_install = "npm install -g node-gyp".to_string();
    let gyp_symlink = "ln -sf /usr/local/lib/node_modules/node-gyp /usr/lib/node_modules/npm/node_modules/node-gyp".to_string();

    if !build.build.commands.contains(&gyp_install) {
        build.build.commands.insert(0, gyp_symlink);
        build.build.commands.insert(0, gyp_install);
    }
}

/// Known Node.js packages that require native compilation (node-gyp).
const NODE_NATIVE_DEPS: &[&str] = &[
    "better-sqlite3",
    "sqlite3",
    "canvas",
    "sharp",
    "bcrypt",
    "node-sass",
    "node-gyp",
    "bufferutil",
    "utf-8-validate",
    "msgpackr-extract",
    "cpu-features",
    "unix-dgram",
    "keytar",
    "re2",
    "farmhash",
    "libxmljs",
    "libxmljs2",
    "node-expat",
    "microtime",
    "couchbase",
    "zeromq",
];

/// Scan Node.js dependencies and add build-base + python when native deps are detected.
pub(crate) fn scan_node_native_deps(repo_root: &Path, build: &mut UniversalBuild) {
    if !matches!(
        build.metadata.language.as_str(),
        "JavaScript" | "TypeScript"
    ) {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let pkg_json_path = project_dir.join("package.json");

    let content = match std::fs::read_to_string(&pkg_json_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return,
    };

    let dep_names: Vec<String> = ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .filter_map(|section| json.get(section)?.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();

    let has_native = dep_names
        .iter()
        .any(|name| NODE_NATIVE_DEPS.iter().any(|pat| name == pat));

    if !has_native {
        return;
    }

    debug!("Adding build-base, python, and node-gyp for Node.js native dependencies");

    for pkg in &["build-base", "python"] {
        let pkg_str = pkg.to_string();
        if !build.build.packages.contains(&pkg_str) {
            build.build.packages.push(pkg_str);
        }
    }

    let gyp_cmd = "npm install -g node-gyp".to_string();
    if !build.build.commands.contains(&gyp_cmd) {
        build.build.commands.insert(0, gyp_cmd.clone());
    }

    let is_pnpm = build.metadata.build_system == "pnpm";
    if is_pnpm {
        let symlink_cmd = "mkdir -p /usr/lib/node_modules/pnpm/dist/node_modules && ln -sf /usr/local/lib/node_modules/node-gyp /usr/lib/node_modules/pnpm/dist/node_modules/node-gyp".to_string();
        if !build.build.commands.contains(&symlink_cmd) {
            let gyp_idx = build
                .build
                .commands
                .iter()
                .position(|c| c.contains("npm install -g node-gyp"))
                .unwrap_or(0);
            build.build.commands.insert(gyp_idx + 1, symlink_cmd);
        }
    }

    scan_node_system_deps(&dep_names, build);
}

/// Maps well-known Node.js packages to their required system build/runtime packages.
const NODE_SYSTEM_DEPS: &[(&str, &[&str], &[&str])] = &[
    (
        "canvas",
        &[
            "cairo-dev",
            "pango-dev",
            "libjpeg-turbo-dev",
            "giflib-dev",
            "pixman-dev",
        ],
        &[
            "cairo",
            "pango",
            "libjpeg-turbo",
            "giflib",
            "pixman",
            "libuuid",
        ],
    ),
    ("sharp", &["vips-dev"], &["vips"]),
    ("better-sqlite3", &["sqlite-dev"], &["sqlite-libs"]),
    ("sqlite3", &["sqlite-dev"], &["sqlite-libs"]),
];

/// Add system library dependencies for specific Node.js native packages.
pub(crate) fn scan_node_system_deps(dep_names: &[String], build: &mut UniversalBuild) {
    for (pattern, build_deps, runtime_deps) in NODE_SYSTEM_DEPS {
        if dep_names.iter().any(|d| d == pattern) {
            for pkg in *build_deps {
                let pkg_str = pkg.to_string();
                if !build.build.packages.contains(&pkg_str) {
                    build.build.packages.push(pkg_str);
                }
            }
            for pkg in *runtime_deps {
                let pkg_str = pkg.to_string();
                if !build.runtime.packages.contains(&pkg_str) {
                    build.runtime.packages.push(pkg_str);
                }
            }
        }
    }
}

// ── Puppeteer / Playwright browser detection ─────────────────────────────

/// Known Puppeteer/Playwright dependency names.
const BROWSER_AUTOMATION_DEPS: &[&str] = &[
    "puppeteer",
    "puppeteer-core",
    "playwright",
    "playwright-core",
    "@playwright/test",
];

/// Scan for Puppeteer/Playwright and add Chromium + related packages.
pub(crate) fn scan_node_puppeteer(repo_root: &Path, build: &mut UniversalBuild) {
    if !matches!(
        build.metadata.language.as_str(),
        "JavaScript" | "TypeScript"
    ) {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let pkg_json_path = project_dir.join("package.json");

    let content = match std::fs::read_to_string(&pkg_json_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return,
    };

    let dep_names: Vec<String> = ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .filter_map(|section| json.get(section)?.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();

    let has_browser = dep_names
        .iter()
        .any(|name| BROWSER_AUTOMATION_DEPS.iter().any(|pat| name == pat));

    if !has_browser {
        return;
    }

    debug!("Adding Chromium packages for Puppeteer/Playwright");

    let browser_build_pkgs = [
        "chromium",
        "nss",
        "freetype",
        "harfbuzz",
        "font-freefont",
        "font-liberation",
    ];

    for pkg in &browser_build_pkgs {
        let pkg_str = pkg.to_string();
        if !build.build.packages.contains(&pkg_str) {
            build.build.packages.push(pkg_str);
        }
    }

    for pkg in &browser_build_pkgs {
        let pkg_str = pkg.to_string();
        if !build.runtime.packages.contains(&pkg_str) {
            build.runtime.packages.push(pkg_str);
        }
    }

    build
        .build
        .env
        .insert("PUPPETEER_SKIP_CHROMIUM_DOWNLOAD".into(), "true".into());
    build.runtime.env.insert(
        "PUPPETEER_EXECUTABLE_PATH".into(),
        "/usr/bin/chromium-browser".into(),
    );
}

// ── Node.js build command sanitization ───────────────────────────────────

/// Patterns in build scripts that are deploy-time operations.
const NODE_BUILD_STRIP_PATTERNS: &[&str] = &[
    "prisma migrate deploy",
    "prisma migrate dev",
    "prisma db push",
    "drizzle-kit push",
    "typeorm migration:run",
    "knex migrate:latest",
];

/// Build tool keywords — if a script contains any of these, it's a real build script.
const NODE_BUILD_TOOL_KEYWORDS: &[&str] = &[
    "tsc",
    "webpack",
    "vite",
    "esbuild",
    "rollup",
    "next build",
    "nuxt build",
    "remix build",
    "react-scripts build",
    "ng build",
    "nest build",
    "prisma generate",
];

/// Sanitize Node.js build commands by removing deploy-time subcommands
/// and dropping env-var-dependent scripts that aren't real build steps.
pub(crate) fn sanitize_node_build_commands(repo_root: &Path, build: &mut UniversalBuild) {
    if !matches!(
        build.metadata.language.as_str(),
        "JavaScript" | "TypeScript"
    ) {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let pkg_json_path = project_dir.join("package.json");

    let content = match std::fs::read_to_string(&pkg_json_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return,
    };

    let build_script = match json
        .get("scripts")
        .and_then(|s| s.get("build"))
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => return,
    };

    let build_cmd_idx = build.build.commands.iter().position(|c| {
        c.contains("run build")
            || c == "pnpm build"
            || c.ends_with("&& pnpm build")
            || c.ends_with("&& pnpm run build")
    });

    let build_cmd_idx = match build_cmd_idx {
        Some(i) => i,
        None => return,
    };

    let needs_strip = NODE_BUILD_STRIP_PATTERNS
        .iter()
        .any(|pat| build_script.contains(pat));

    if needs_strip {
        let parts: Vec<&str> = build_script.split("&&").map(|s| s.trim()).collect();
        let filtered: Vec<&str> = parts
            .iter()
            .filter(|part| {
                !NODE_BUILD_STRIP_PATTERNS
                    .iter()
                    .any(|pat| part.contains(pat))
            })
            .copied()
            .collect();

        if filtered.is_empty() {
            build.build.commands.remove(build_cmd_idx);
        } else {
            let sanitized = filtered.join(" && ");
            if sanitized != build_script {
                let escaped_sanitized = sanitized.replace('\\', "\\\\").replace('\"', "\\\"");
                let rewrite_cmd = format!(
                    "node -e \"var p=require('./package.json');var f=require('fs');p.scripts.build='{}';f.writeFileSync('./package.json',JSON.stringify(p,null,2)+'\\n')\"",
                    escaped_sanitized
                );
                build.build.commands.insert(build_cmd_idx, rewrite_cmd);
            }
        }
        return;
    }

    let has_env_var = build_script.contains("$") || build_script.contains("${");
    let has_build_tool = NODE_BUILD_TOOL_KEYWORDS
        .iter()
        .any(|kw| build_script.contains(kw));

    if has_env_var && !has_build_tool {
        build.build.commands.remove(build_cmd_idx);
    }
}

// ── Framework fallback entrypoints ───────────────────────────────────────

/// For detected frameworks without an explicit start script, provide a
/// production-ready fallback entrypoint command.
pub(crate) fn provide_framework_fallback_entrypoint(build: &mut UniversalBuild) {
    if !build.runtime.command.is_empty() {
        return;
    }
    let framework = match build.metadata.framework.as_deref() {
        Some(fw) => fw,
        None => return,
    };

    let exec_prefix: Vec<&str> = match build.metadata.build_system.as_str() {
        "Bun" => vec!["bunx"],
        "pnpm" => vec!["pnpm", "exec"],
        "Yarn" => vec!["yarn"],
        _ => vec!["npx"],
    };

    let with_exec = |args: &[&str]| -> Vec<String> {
        exec_prefix
            .iter()
            .chain(args.iter())
            .map(|s| s.to_string())
            .collect()
    };

    let command: Vec<String> = match framework {
        "Vite" => with_exec(&["vite", "preview", "--host", "0.0.0.0"]),
        "Create React App" => with_exec(&["serve", "-s", "build"]),
        "Angular" => with_exec(&["serve", "-s", "dist/browser"]),
        "Gatsby" => with_exec(&["gatsby", "serve", "-H", "0.0.0.0"]),
        "Nuxt" => vec!["node", ".output/server/index.mjs"]
            .into_iter()
            .map(String::from)
            .collect(),
        "Astro" => with_exec(&["astro", "preview", "--host", "0.0.0.0"]),
        "SvelteKit" => vec!["node", "build/index.js"]
            .into_iter()
            .map(String::from)
            .collect(),
        "React Router" => with_exec(&["react-router-serve", "build/server/index.js"]),
        "Next.js" => with_exec(&["next", "start"]),
        "Remix" => with_exec(&["remix-serve", "build/server/index.js"]),
        "SolidStart" => vec!["node", ".output/server/index.mjs"]
            .into_iter()
            .map(String::from)
            .collect(),
        "TanStack Start" => vec!["node_modules/.bin/h3", "dist/server/server.js"]
            .into_iter()
            .map(String::from)
            .collect(),
        "Docusaurus" => with_exec(&["docusaurus", "serve"]),
        _ => return,
    };

    debug!(
        framework = framework,
        command = ?command,
        "Providing framework fallback entrypoint"
    );
    build.runtime.command = command;
}

/// When React Router is in SPA mode (ssr: false), switch to Vite preview.
pub(crate) fn detect_react_router_spa(repo_path: &Path, build: &mut UniversalBuild) {
    if build.metadata.framework.as_deref() != Some("React Router") {
        return;
    }
    let project_dir = if let Some(name) = &build.metadata.project_name {
        let candidate = repo_path.join(name);
        if candidate.is_dir() {
            candidate
        } else {
            repo_path.to_path_buf()
        }
    } else {
        repo_path.to_path_buf()
    };

    for config_name in &["react-router.config.ts", "react-router.config.js"] {
        let config_path = project_dir.join(config_name);
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if content.contains("ssr: false") || content.contains("ssr:false") {
                debug!(
                    project = build.metadata.project_name.as_deref().unwrap_or("?"),
                    "Detected React Router SPA mode (ssr: false), switching to vite preview"
                );
                let exec_prefix: Vec<&str> = match build.metadata.build_system.as_str() {
                    "Bun" => vec!["bunx"],
                    "pnpm" => vec!["pnpm", "exec"],
                    "Yarn" => vec!["yarn"],
                    _ => vec!["npx"],
                };
                build.runtime.command = exec_prefix
                    .iter()
                    .chain(["vite", "preview", "--host", "0.0.0.0"].iter())
                    .map(|s| s.to_string())
                    .collect();
                if build.runtime.ports == vec![3000] {
                    build.runtime.ports = vec![4173];
                }
                return;
            }
        }
    }
}

/// For Yarn >= 2 (Berry) projects, wrap the runtime entrypoint with `corepack enable`.
pub(crate) fn wrap_yarn_corepack_entrypoint(build: &mut UniversalBuild) {
    if build.metadata.build_system != "Yarn" {
        return;
    }
    if !build.runtime.packages.iter().any(|p| p == "corepack") {
        return;
    }
    if build.runtime.command.is_empty() {
        return;
    }
    if build.runtime.command.first().map(|s| s.as_str()) == Some("sh") {
        return;
    }
    let original_cmd = build.runtime.command.join(" ");
    build.runtime.command = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("corepack enable && exec {}", original_cmd),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node_build(
        build_packages: Vec<String>,
        runtime_packages: Vec<String>,
        language: &str,
    ) -> UniversalBuild {
        UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: language.into(),
                build_system: "npm".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: build_packages,
                commands: vec!["npm install".into()],
                env: BTreeMap::new(),
                cache: vec![".npm".into()],
            },
            runtime: peelbox_core::output::schema::RuntimeStage {
                packages: runtime_packages,
                command: vec!["node".into(), "index.js".into()],
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_resolve_node_version_available_in_wolfi() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-22".into(), "npm".into()],
            vec!["nodejs-22".into()],
            "JavaScript",
        );

        resolve_node_version(&mut build, &wolfi);

        // nodejs-22 exists in Wolfi -- no changes
        assert_eq!(build.build.packages[0], "nodejs-22");
        assert_eq!(build.build.commands.len(), 1);
    }

    #[test]
    fn test_resolve_node_version_not_in_wolfi() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-14".into(), "npm".into()],
            vec!["nodejs-14".into(), "npm".into()],
            "JavaScript",
        );

        resolve_node_version(&mut build, &wolfi);

        assert_eq!(build.build.packages[0], "curl");
        assert!(!build.build.packages.contains(&"npm".to_string()));
        assert!(build
            .build
            .packages
            .contains(&"ca-certificates".to_string()));
        assert!(build.build.packages.contains(&"build-base".to_string()));
        assert!(build.build.packages.contains(&"bash".to_string()));
        assert!(build.build.packages.contains(&"libstdc++".to_string()));
        assert_eq!(build.build.commands.len(), 2);
        assert!(build.build.commands[0].contains("mkdir -p /usr/local/bin"));
        assert!(build.build.commands[0].contains("/n"));
        assert!(build.build.commands[0].ends_with(" 14"));
        assert_eq!(build.build.commands[1], "npm install");
        assert!(build.build.env.contains_key("PATH"));
    }

    #[test]
    fn test_resolve_node_version_fixes_runtime() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-14".into(), "npm".into()],
            vec!["nodejs-14".into(), "npm".into()],
            "JavaScript",
        );

        resolve_node_version(&mut build, &wolfi);

        assert!(build.runtime.packages.contains(&"curl".to_string()));
        assert!(!build.runtime.packages.contains(&"npm".to_string()));
        assert!(build
            .runtime
            .packages
            .contains(&"ca-certificates".to_string()));

        assert_eq!(build.runtime.command[0], "sh");
        assert_eq!(build.runtime.command[1], "-c");
        assert!(build.runtime.command[2].contains("node index.js"));
        assert!(build.runtime.command[2].contains("/n"));
    }

    #[test]
    fn test_resolve_node_version_skips_non_js() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: vec!["python-3.11".into()],
                commands: vec!["pip install .".into()],
                env: BTreeMap::new(),
                cache: vec![],
            },
            runtime: Default::default(),
        };

        resolve_node_version(&mut build, &wolfi);

        assert_eq!(build.build.packages[0], "python-3.11");
        assert_eq!(build.build.commands.len(), 1);
    }

    #[test]
    fn test_resolve_node_version_typescript() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-14".into(), "npm".into()],
            vec!["nodejs-14".into(), "npm".into()],
            "TypeScript",
        );

        resolve_node_version(&mut build, &wolfi);

        assert_eq!(build.build.packages[0], "curl");
        assert!(build.build.commands[0].ends_with(" 14"));
    }

    #[test]
    fn test_resolve_node_16_not_affected() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-16".into(), "npm".into()],
            vec!["nodejs-16".into()],
            "JavaScript",
        );
        let original_packages = build.build.packages.clone();

        resolve_node_version(&mut build, &wolfi);

        if wolfi.has_package("nodejs-16") {
            assert_eq!(build.build.packages, original_packages);
        }
    }

    #[test]
    fn test_parse_node_version_string() {
        assert_eq!(
            parse_node_version_string("v18.12.0"),
            Some("18".to_string())
        );
        assert_eq!(parse_node_version_string("20"), Some("20".to_string()));
        assert_eq!(parse_node_version_string("18.12.0"), Some("18".to_string()));
        assert_eq!(
            parse_node_version_string("lts/iron"),
            Some("20".to_string())
        );
        assert_eq!(
            parse_node_version_string("lts/hydrogen"),
            Some("18".to_string())
        );
        assert_eq!(parse_node_version_string("lts/*"), None);
    }

    #[test]
    fn test_read_node_version_from_nvmrc() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".nvmrc"), "v18.12.0\n").unwrap();
        assert_eq!(
            read_node_version(dir.path(), dir.path()),
            Some("18".to_string())
        );
    }

    #[test]
    fn test_read_node_version_from_node_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".node-version"), "20\n").unwrap();
        assert_eq!(
            read_node_version(dir.path(), dir.path()),
            Some("20".to_string())
        );
    }

    #[test]
    fn test_read_node_version_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_node_version(dir.path(), dir.path()), None);
    }

    #[test]
    fn test_node_tooling_floor_pnpm_bumps_to_22() {
        let mut build = make_node_build(
            vec!["nodejs-20".into(), "pnpm".into()],
            vec!["nodejs-20".into(), "pnpm".into()],
            "JavaScript",
        );
        build.metadata.build_system = "pnpm".into();

        ensure_node_tooling_floor(&mut build);

        assert!(build.build.packages.contains(&"nodejs-22".to_string()));
        assert!(build.runtime.packages.contains(&"nodejs-22".to_string()));
        assert!(!build.build.packages.contains(&"nodejs-20".to_string()));
    }

    #[test]
    fn test_node_tooling_floor_npm_bumps_16_to_18() {
        let mut build = make_node_build(
            vec!["nodejs-16".into(), "npm".into()],
            vec!["nodejs-16".into()],
            "JavaScript",
        );
        // build_system defaults to "npm" in make_node_build

        ensure_node_tooling_floor(&mut build);

        assert!(build.build.packages.contains(&"nodejs-18".to_string()));
    }

    #[test]
    fn test_node_tooling_floor_handles_capitalized_yarn_with_npm() {
        // Yarn Berry pulls in npm (corepack/node-gyp); the serialized build system
        // is capitalized ("Yarn"), so the match must be case-insensitive.
        let mut build =
            make_node_build(vec!["nodejs-16".into(), "npm".into()], vec![], "JavaScript");
        build.metadata.build_system = "Yarn".into();

        ensure_node_tooling_floor(&mut build);

        assert!(build.build.packages.contains(&"nodejs-18".to_string()));
    }

    #[test]
    fn test_node_tooling_floor_classic_yarn_not_bumped() {
        // Classic yarn (no npm package) strictly enforces engines.node, so its
        // pinned Node version must be left alone.
        let mut build = make_node_build(vec!["nodejs-16".into()], vec![], "JavaScript");
        build.metadata.build_system = "Yarn".into();

        ensure_node_tooling_floor(&mut build);

        assert!(build.build.packages.contains(&"nodejs-16".to_string()));
    }

    #[test]
    fn test_node_tooling_floor_leaves_n_installer_versions() {
        // Versions below the minimum Wolfi major use the `n` installer with the
        // Node-bundled npm and must not be floored.
        let mut build = make_node_build(vec!["nodejs-14".into()], vec![], "JavaScript");
        build.metadata.build_system = "pnpm".into();

        ensure_node_tooling_floor(&mut build);

        assert!(build.build.packages.contains(&"nodejs-14".to_string()));
    }

    #[test]
    fn test_node_tooling_floor_no_downgrade() {
        let mut build = make_node_build(vec!["nodejs-24".into()], vec![], "JavaScript");
        build.metadata.build_system = "pnpm".into();

        ensure_node_tooling_floor(&mut build);

        assert!(build.build.packages.contains(&"nodejs-24".to_string()));
    }

    #[test]
    fn test_pnpm_allow_builds_prepended() {
        let mut build = make_node_build(
            vec!["nodejs-22".into(), "pnpm".into()],
            vec![],
            "JavaScript",
        );
        build.metadata.build_system = "pnpm".into();
        build.build.commands = vec!["pnpm install".into(), "pnpm run build".into()];

        ensure_pnpm_allow_builds(&mut build);

        assert_eq!(
            build.build.commands[0],
            "pnpm config set dangerously-allow-all-builds true"
        );
        // Idempotent — running again does not add a second config command.
        ensure_pnpm_allow_builds(&mut build);
        assert_eq!(
            build
                .build
                .commands
                .iter()
                .filter(|c| c.contains("dangerously-allow-all-builds"))
                .count(),
            1
        );
    }

    #[test]
    fn test_pnpm_allow_builds_skips_npm() {
        let mut build =
            make_node_build(vec!["nodejs-22".into(), "npm".into()], vec![], "JavaScript");
        build.build.commands = vec!["npm ci".into()];

        ensure_pnpm_allow_builds(&mut build);

        assert!(!build
            .build
            .commands
            .iter()
            .any(|c| c.contains("dangerously-allow-all-builds")));
    }
}

//! Framework-level post-processing: fallback entrypoints and Yarn Berry
//! corepack wrapping.

use peelbox_core::output::schema::UniversalBuild;
use tracing::debug;

// ── Framework fallback entrypoints ───────────────────────────────────────

/// For detected frameworks without an explicit start script, provide a
/// production-ready fallback entrypoint command. This only triggers when
/// the build has a detected framework but no runtime command (i.e., no
/// scripts.start, no main field, no Procfile).
pub fn provide_framework_fallback_entrypoint(build: &mut UniversalBuild) {
    if !build.runtime.command.is_empty() {
        return;
    }
    let framework = match build.metadata.framework.as_deref() {
        Some(fw) => fw,
        None => return,
    };

    // Build the exec prefix based on package manager.
    // npx is only available when npm is installed; pnpm/yarn use their own exec commands.
    let exec_prefix: Vec<&str> = match build.metadata.build_system.as_str() {
        "Bun" => vec!["bunx"],
        "pnpm" => vec!["pnpm", "exec"],
        "Yarn" => vec!["yarn"],
        _ => vec!["npx"],
    };

    // Helper: build a command with the package-manager exec prefix
    let with_exec = |args: &[&str]| -> Vec<String> {
        exec_prefix
            .iter()
            .chain(args.iter())
            .map(|s| s.to_string())
            .collect()
    };

    let command: Vec<String> = match framework {
        // SPA / static build frameworks — serve built output
        "Vite" => with_exec(&["vite", "preview", "--host", "0.0.0.0"]),
        "Create React App" => with_exec(&["serve", "-s", "build"]),
        "Angular" => with_exec(&["serve", "-s", "dist/browser"]),
        "Gatsby" => with_exec(&["gatsby", "serve", "-H", "0.0.0.0"]),

        // SSR / full-stack frameworks — run the built server
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

// ── React Router SPA mode detection ─────────────────────────────────────

/// When React Router is in SPA mode (ssr: false in react-router.config.ts),
/// `react-router-serve` won't work because `build/server/index.js` doesn't exist.
/// Switch to Vite preview instead.
pub fn detect_react_router_spa(repo_path: &std::path::Path, build: &mut UniversalBuild) {
    if build.metadata.framework.as_deref() != Some("React Router") {
        return;
    }
    // Check for react-router.config.ts with ssr: false
    let _workdir = &build.runtime.workdir;
    // Resolve the project directory within the repo
    let project_dir = if let Some(name) = &build.metadata.project_name {
        // For workspace members, the config might be in a subdirectory
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
                // Use Vite preview for SPA mode
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
                // Vite preview uses port 4173
                if build.runtime.ports == vec![3000] {
                    build.runtime.ports = vec![4173];
                }
                return;
            }
        }
    }
}

// ── Yarn Berry corepack entrypoint wrapping ─────────────────────────────

/// For Yarn >= 2 (Berry) projects, the runtime entrypoint needs `corepack enable`
/// before `yarn start` because the Wolfi `yarn` package provides Yarn 1.x.
/// Corepack reads the `packageManager` field from package.json and uses the
/// correct Yarn version.
pub fn wrap_yarn_corepack_entrypoint(build: &mut UniversalBuild) {
    if build.metadata.build_system != "Yarn" {
        return;
    }
    // Yarn Berry is identified by having corepack in runtime packages
    if !build.runtime.packages.iter().any(|p| p == "corepack") {
        return;
    }
    if build.runtime.command.is_empty() {
        return;
    }
    // Already wrapped
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

//! Node.js-specific post-processing: native dependency detection,
//! Puppeteer/Playwright browser setup, and build command sanitization.

use crate::helpers::extract_project_dir;

use peelbox_core::output::schema::UniversalBuild;
use std::path::Path;
use tracing::debug;

// ── Node.js native dependency detection ──────────────────────────────────

/// Known Node.js packages that require native compilation (node-gyp).
/// When detected, build-base and python are added as build packages.
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
pub fn scan_node_native_deps(repo_root: &Path, build: &mut UniversalBuild) {
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

    // Collect all dependency names from dependencies, devDependencies, optionalDependencies
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

    // Install node-gyp globally so native modules can be compiled.
    // node-gyp is not available as a Wolfi package so we install it via npm.
    let gyp_cmd = "npm install -g node-gyp".to_string();
    if !build.build.commands.contains(&gyp_cmd) {
        // Insert before the install command so node-gyp is available during npm ci / pnpm install
        build.build.commands.insert(0, gyp_cmd);
    }

    // pnpm bundles its own node-gyp reference at a fixed path inside its distribution.
    // When the Wolfi pnpm package doesn't include node-gyp, pnpm's postinstall
    // scripts fail with MODULE_NOT_FOUND. Symlink the globally installed node-gyp
    // into the path pnpm expects.
    let is_pnpm = build.metadata.build_system == "pnpm";
    if is_pnpm {
        let symlink_cmd = "mkdir -p /usr/lib/node_modules/pnpm/dist/node_modules && ln -sf /usr/local/lib/node_modules/node-gyp /usr/lib/node_modules/pnpm/dist/node_modules/node-gyp".to_string();
        if !build.build.commands.contains(&symlink_cmd) {
            // Insert after node-gyp install but before the package install command
            let gyp_idx = build
                .build
                .commands
                .iter()
                .position(|c| c.contains("npm install -g node-gyp"))
                .unwrap_or(0);
            build.build.commands.insert(gyp_idx + 1, symlink_cmd);
        }
    }

    // Add package-specific system library dependencies.
    // Some Node.js native modules need specific C libraries beyond just build-base.
    scan_node_system_deps(&dep_names, build);
}

/// Maps well-known Node.js packages to their required system build/runtime packages.
/// Each entry: (npm_package, build_packages, runtime_packages).
const NODE_SYSTEM_DEPS: &[(&str, &[&str], &[&str])] = &[
    // canvas (node-canvas) requires Cairo, Pango, image format libraries, and libuuid
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
    // sharp requires vips (image processing library)
    ("sharp", &["vips-dev"], &["vips"]),
    // better-sqlite3 and sqlite3 need SQLite headers
    ("better-sqlite3", &["sqlite-dev"], &["sqlite-libs"]),
    ("sqlite3", &["sqlite-dev"], &["sqlite-libs"]),
];

/// Add system library dependencies for specific Node.js native packages.
pub fn scan_node_system_deps(dep_names: &[String], build: &mut UniversalBuild) {
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
pub fn scan_node_puppeteer(repo_root: &Path, build: &mut UniversalBuild) {
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

    // Runtime also needs Chromium and related packages
    for pkg in &browser_build_pkgs {
        let pkg_str = pkg.to_string();
        if !build.runtime.packages.contains(&pkg_str) {
            build.runtime.packages.push(pkg_str);
        }
    }

    // Set PUPPETEER_SKIP_CHROMIUM_DOWNLOAD to avoid downloading
    // Chromium during npm install (we use the system-installed one)
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

/// Patterns in build scripts that are deploy-time operations (DB migrations,
/// etc.) and should be stripped from the build phase.
const NODE_BUILD_STRIP_PATTERNS: &[&str] = &[
    "prisma migrate deploy",
    "prisma migrate dev",
    "prisma db push",
    "drizzle-kit push",
    "typeorm migration:run",
    "knex migrate:latest",
];

/// Build tool keywords — if a script contains any of these, it's a real
/// build script even if it also references env vars.
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
/// (like `prisma migrate deploy`) and dropping env-var-dependent scripts
/// that aren't real build steps.
pub fn sanitize_node_build_commands(repo_root: &Path, build: &mut UniversalBuild) {
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

    // Find the build command in the commands list (e.g., "npm run build", "yarn run build", "pnpm build")
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

    // Mode 1: Strip deploy-time subcommands from the build script
    let needs_strip = NODE_BUILD_STRIP_PATTERNS
        .iter()
        .any(|pat| build_script.contains(pat));

    if needs_strip {
        // Split on && and filter out deploy-time commands
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
            // All parts were deploy-time; remove the build command entirely
            build.build.commands.remove(build_cmd_idx);
        } else {
            let sanitized = filtered.join(" && ");
            if sanitized != build_script {
                // Rewrite package.json's build script in-place before running
                // the package manager's build command. This preserves
                // node_modules/.bin PATH resolution.
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

    // Mode 2: Drop env-var-dependent build scripts that aren't real build steps
    let has_env_var = build_script.contains("$") || build_script.contains("${");
    let has_build_tool = NODE_BUILD_TOOL_KEYWORDS
        .iter()
        .any(|kw| build_script.contains(kw));

    if has_env_var && !has_build_tool {
        build.build.commands.remove(build_cmd_idx);
    }
}

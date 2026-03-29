//! Map-Reduce detection pipeline.
//!
//! Four steps: Parse → Detect Framework → Partition → Reduce.
//!
//! Build-system-specific behavior is delegated to `BuildSystemConfig` profiles
//! registered via `inventory::submit!` in the parser files.

use crate::helpers::{extract_project_dir, replace_package};
use crate::registry::Registry;
use crate::source_scanning::{scan_source_env_vars, scan_source_health, scan_source_ports};
use crate::traits::{ConfigParser, ManifestParser};
use crate::types::*;
use crate::version::mise::scan_mise_config;
use crate::version::node::read_node_version;
use crate::version::php::read_php_version;
use crate::version::python::read_python_version;
use crate::version::ruby::read_ruby_version;
use crate::version::swift::read_swift_version;

use anyhow::Result;
use ignore::WalkBuilder;
use peelbox_core::output::schema::{
    BuildMetadata, BuildStage, CopySpec, HealthCheck, RuntimeStage, UniversalBuild,
};
use peelbox_wolfi::WolfiPackageIndex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Run the full detection pipeline on a repository.
/// Resolves Wolfi package versions automatically.
pub fn detect(repo_path: &Path) -> Result<Vec<UniversalBuild>> {
    let registry = Registry::with_defaults();
    let wolfi_index = WolfiPackageIndex::fetch()?;
    detect_with_registry_and_wolfi(repo_path, &registry, Some(&wolfi_index))
}

/// Run without Wolfi resolution (for testing).
pub fn detect_without_wolfi(repo_path: &Path) -> Result<Vec<UniversalBuild>> {
    let registry = Registry::with_defaults();
    detect_with_registry_and_wolfi(repo_path, &registry, None)
}

/// Run the full detection pipeline with a custom registry.
pub fn detect_with_registry(repo_path: &Path, registry: &Registry) -> Result<Vec<UniversalBuild>> {
    detect_with_registry_and_wolfi(repo_path, registry, None)
}

/// Run the full detection pipeline with a custom registry and optional Wolfi index.
pub fn detect_with_registry_and_wolfi(
    repo_path: &Path,
    registry: &Registry,
    wolfi_index: Option<&WolfiPackageIndex>,
) -> Result<Vec<UniversalBuild>> {
    info!(repo = %repo_path.display(), "Starting map-reduce detection pipeline");

    // Step 1: Walk filesystem and parse into typed tree
    let repo_tree = build_tree(repo_path, registry)?;

    // Step 2: Framework detection on all manifests
    let manifests_with_frameworks = detect_frameworks(&repo_tree, registry);

    // Step 3: Partition into service buckets
    let buckets = partition(&repo_tree, manifests_with_frameworks, registry);

    info!(services = buckets.len(), "Partitioned into service buckets");

    // Step 4: Reduce each bucket into UniversalBuild
    let mut builds: Vec<UniversalBuild> = buckets
        .into_iter()
        .filter_map(|bucket| match reduce(bucket, registry) {
            Ok(build) => Some(build),
            Err(e) => {
                warn!("Failed to reduce service bucket: {}", e);
                None
            }
        })
        .collect();

    // Step 4b: Scan source code for port patterns
    for build in &mut builds {
        scan_source_ports(repo_path, build);
    }

    // Step 4c: Scan source code for health endpoints
    for build in &mut builds {
        scan_source_health(repo_path, build);
    }

    // Step 4d: Scan source code for environment variables
    for build in &mut builds {
        scan_source_env_vars(repo_path, build);
    }

    // Step 4e: Scan version files (.nvmrc, .python-version, etc.)
    for build in &mut builds {
        scan_version_files(repo_path, build);
    }

    // Step 4e2: Scan mise.toml / .mise.toml / .tool-versions for tool version overrides
    for build in &mut builds {
        scan_mise_config(repo_path, build);
    }

    // Step 4f: Scan Python entrypoints
    for build in &mut builds {
        scan_python_entrypoints(repo_path, build);
    }

    // Step 4g: Fix FLASK_APP for workspace members where hardcoded path is wrong
    for build in &mut builds {
        fix_flask_app_path(repo_path, build);
    }

    // Step 4h: Detect Django settings module from manage.py
    for build in &mut builds {
        fix_django_settings(repo_path, build);
    }

    // Step 4i: Detect Python native dependency system packages
    for build in &mut builds {
        scan_python_native_deps(repo_path, build);
    }

    // Step 4j: Detect Node.js native dependency system packages
    for build in &mut builds {
        scan_node_native_deps(repo_path, build);
    }

    // Step 4k: Detect Puppeteer and add Chromium dependencies
    for build in &mut builds {
        scan_node_puppeteer(repo_path, build);
    }

    // Step 4l: Sanitize Node.js build commands (remove DB-dependent steps, etc.)
    for build in &mut builds {
        sanitize_node_build_commands(repo_path, build);
    }

    // Step 5: Resolve Wolfi package versions
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            resolve_wolfi_packages(&mut build.build.packages, wolfi);
            resolve_wolfi_packages(&mut build.runtime.packages, wolfi);
        }
    }

    // Step 5b: Sync JAVA_HOME with resolved openjdk package version
    for build in &mut builds {
        crate::version::java::sync_java_home_with_packages(build);
    }

    // Step 6: Handle pinned versions not available in Wolfi (use alternative installers)
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            crate::version::rust::resolve_rust_toolchain(build, wolfi);
            crate::version::node::resolve_node_version(build, wolfi);
        }
    }

    // Step 7: Handle old Java versions not available in Wolfi (use Adoptium Temurin)
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            crate::version::java::resolve_java_toolchain(build, wolfi);
        }
    }

    // Step 7b: Set PORT env var for JVM apps when runtime has ports.
    // Many Spring/JVM apps use ${PORT:default} in config; PaaS convention is to set PORT.
    for build in &mut builds {
        let lang = build.metadata.language.as_str();
        if matches!(lang, "Java" | "Kotlin" | "Scala" | "Clojure") {
            if let Some(&port) = build.runtime.ports.first() {
                build
                    .runtime
                    .env
                    .entry("PORT".to_string())
                    .or_insert_with(|| port.to_string());
            }
        }
    }

    // Step 8: Wrap Yarn Berry entrypoints with corepack enable
    for build in &mut builds {
        wrap_yarn_corepack_entrypoint(build);
    }

    // Step 9: Provide fallback entrypoints for detected frameworks without scripts.start
    for build in &mut builds {
        provide_framework_fallback_entrypoint(build);
    }

    // Filter out non-application builds (e.g., library crates, utility packages)
    builds.retain(|b| !b.runtime.command.is_empty());

    info!(builds = builds.len(), "Detection pipeline complete");
    Ok(builds)
}

// ── Step 1: Build Tree ──────────────────────────────────────────────────────

fn build_tree(repo_path: &Path, registry: &Registry) -> Result<RepoTree> {
    let mut file_map: HashMap<PathBuf, Vec<TypedFile>> = HashMap::new();

    // Build filename lookup for parsers
    let manifest_lookup = build_parser_lookup(&registry.manifest_parsers);
    let config_lookup = build_config_lookup(&registry.config_parsers);

    // Walk the filesystem respecting .gitignore
    let walker = WalkBuilder::new(repo_path)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let abs_path = entry.path();
        let rel_path = abs_path
            .strip_prefix(repo_path)
            .unwrap_or(abs_path)
            .to_path_buf();

        let filename = rel_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let dir = rel_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        let kind = classify_file(
            abs_path,
            &rel_path,
            filename,
            &manifest_lookup,
            &config_lookup,
        );

        file_map.entry(dir).or_default().push(TypedFile {
            path: rel_path,
            kind,
        });
    }

    // Build hierarchical tree from flat map
    let tree = build_dir_node(Path::new(""), &mut file_map);

    Ok(RepoTree {
        root: repo_path.to_path_buf(),
        tree,
    })
}

fn build_parser_lookup(parsers: &[Box<dyn ManifestParser>]) -> HashMap<&str, &dyn ManifestParser> {
    let mut map = HashMap::new();
    for parser in parsers {
        for filename in parser.filenames() {
            map.insert(*filename, parser.as_ref());
        }
    }
    map
}

fn build_config_lookup(parsers: &[Box<dyn ConfigParser>]) -> HashMap<&str, &dyn ConfigParser> {
    let mut map = HashMap::new();
    for parser in parsers {
        for filename in parser.filenames() {
            map.insert(*filename, parser.as_ref());
        }
    }
    map
}

fn classify_file(
    abs_path: &Path,
    rel_path: &Path,
    filename: &str,
    manifest_lookup: &HashMap<&str, &dyn ManifestParser>,
    config_lookup: &HashMap<&str, &dyn ConfigParser>,
) -> FileKind {
    // Try manifest parsers first
    if let Some(parser) = manifest_lookup.get(filename) {
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if let Some(mut manifest) = parser.parse(abs_path, &content) {
                // Normalize path back to relative
                manifest.path = rel_path.to_path_buf();
                debug!(file = %rel_path.display(), "Parsed manifest");
                return FileKind::Manifest(Box::new(manifest));
            }
        }
    }

    // Try .csproj / .fsproj files (special case: extension-based matching)
    if filename.ends_with(".csproj") || filename.ends_with(".fsproj") {
        let csproj_parser = crate::parsers::manifest::CsprojParser;
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if let Some(mut manifest) = ManifestParser::parse(&csproj_parser, abs_path, &content) {
                manifest.path = rel_path.to_path_buf();
                debug!(file = %rel_path.display(), "Parsed .NET project file");
                return FileKind::Manifest(Box::new(manifest));
            }
        }
    }

    // Try .cbl files (special case: extension-based matching for COBOL)
    if filename.ends_with(".cbl") {
        let cobol_parser = crate::parsers::manifest::CobolParser;
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if let Some(mut manifest) = ManifestParser::parse(&cobol_parser, abs_path, &content) {
                manifest.path = rel_path.to_path_buf();
                debug!(file = %rel_path.display(), "Parsed COBOL source file");
                return FileKind::Manifest(Box::new(manifest));
            }
        }
    }

    // Try .cabal files (special case: extension-based matching)
    if filename.ends_with(".cabal") {
        let cabal_parser = crate::parsers::manifest::CabalFileParser;
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if let Some(mut manifest) = ManifestParser::parse(&cabal_parser, abs_path, &content) {
                manifest.path = rel_path.to_path_buf();
                debug!(file = %rel_path.display(), "Parsed Haskell .cabal file");
                return FileKind::Manifest(Box::new(manifest));
            }
        }
    }

    // Try .ts files for Deno URL imports (e.g., https://deno.land/)
    if filename.ends_with(".ts") && !filename.ends_with(".d.ts") {
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if content.contains("https://deno.land/") || content.contains("jsr:@") {
                // Check no deno.json/deno.jsonc or package.json exists
                let parent = abs_path.parent().unwrap_or(Path::new("."));
                let has_manifest = parent.join("deno.json").exists()
                    || parent.join("deno.jsonc").exists()
                    || parent.join("package.json").exists();
                // Also check repo root (one level up from src/)
                let repo_has_manifest = parent
                    .parent()
                    .map(|p| {
                        p.join("deno.json").exists()
                            || p.join("deno.jsonc").exists()
                            || p.join("package.json").exists()
                    })
                    .unwrap_or(false);
                if !has_manifest && !repo_has_manifest {
                    let deno_parser = crate::parsers::manifest::DenoJsonParser;
                    // Provide a minimal deno.json-like content to the parser
                    let synthetic = r#"{"tasks":{}}"#;
                    if let Some(mut manifest) =
                        ManifestParser::parse(&deno_parser, abs_path, synthetic)
                    {
                        // Override entrypoint to point to the actual .ts file
                        let ts_path = rel_path.display().to_string();
                        manifest.runtime_config.entrypoint = Some(format!(
                            "deno run --allow-net --allow-read --allow-env {}",
                            ts_path
                        ));
                        manifest.path = rel_path.to_path_buf();
                        debug!(file = %rel_path.display(), "Detected Deno from URL imports in .ts file");
                        return FileKind::Manifest(Box::new(manifest));
                    }
                }
            }
        }
    }

    // Try config parsers
    if let Some(parser) = config_lookup.get(filename) {
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if let Some(config) = parser.parse(rel_path, &content) {
                debug!(file = %rel_path.display(), "Parsed config");
                return FileKind::Config(config);
            }
        }
    }

    FileKind::Other
}

fn build_dir_node(dir_path: &Path, file_map: &mut HashMap<PathBuf, Vec<TypedFile>>) -> DirNode {
    let files = file_map.remove(dir_path).unwrap_or_default();

    // Find immediate child directory prefixes
    let mut all_child_prefixes: Vec<PathBuf> = file_map
        .keys()
        .filter_map(|k| {
            if dir_path.as_os_str().is_empty() {
                k.components().next().map(|c| PathBuf::from(c.as_os_str()))
            } else if k.starts_with(dir_path) {
                let rest = k.strip_prefix(dir_path).ok()?;
                rest.components()
                    .next()
                    .map(|c| dir_path.join(c.as_os_str()))
            } else {
                None
            }
        })
        .collect();
    all_child_prefixes.sort();
    all_child_prefixes.dedup();

    let children: Vec<DirNode> = all_child_prefixes
        .into_iter()
        .map(|child_dir| build_dir_node(&child_dir, file_map))
        .collect();

    DirNode {
        path: dir_path.to_path_buf(),
        files,
        children,
    }
}

// ── Step 2: Framework Detection ─────────────────────────────────────────────

struct ManifestWithFramework {
    path: PathBuf,
    manifest: Manifest,
    framework: Option<FrameworkContribution>,
}

fn detect_frameworks(tree: &RepoTree, registry: &Registry) -> Vec<ManifestWithFramework> {
    let mut results = Vec::new();
    collect_manifests_with_frameworks(&tree.tree, registry, &mut results);
    results
}

fn collect_manifests_with_frameworks(
    node: &DirNode,
    registry: &Registry,
    results: &mut Vec<ManifestWithFramework>,
) {
    let detectors = &registry.framework_detectors;

    for file in &node.files {
        if let FileKind::Manifest(manifest) = &file.kind {
            // Collect all matching frameworks, then pick the best one.
            // Prefer frameworks whose trigger dependency is a runtime (production)
            // dependency over those that only match on devDependencies.
            let mut candidates: Vec<(FrameworkContribution, bool)> = Vec::new();
            for detector in detectors.iter() {
                if !detector.compatible_languages().contains(&manifest.language) {
                    continue;
                }
                // Check detection against all deps
                if !detector.detect(&manifest.dependencies) {
                    continue;
                }
                let contrib = detector.contribution(&manifest.dependencies);
                // Check if this framework also matches when restricted to runtime deps only
                let runtime_deps: Vec<_> = manifest
                    .dependencies
                    .iter()
                    .filter(|d| d.scope == DepScope::Runtime)
                    .cloned()
                    .collect();
                let matches_runtime = detector.detect(&runtime_deps);
                candidates.push((contrib, matches_runtime));
            }

            // Sort: runtime matches first, then by order (stable sort preserves insertion order)
            candidates.sort_by_key(|&(_, matches_runtime)| if matches_runtime { 0 } else { 1 });

            let mut framework = candidates.into_iter().next().map(|(contrib, _)| contrib);

            // If the build system profile prefers framework variants with specific env keys,
            // look for a matching variant (e.g., Poetry prefers FlaskPoetry with VIRTUAL_ENV)
            if let Some(profile) = registry.get_profile(&manifest.build_system) {
                let env_keys = profile.preferred_framework_env_keys;
                if !env_keys.is_empty() {
                    if let Some(ref fw) = framework {
                        // Collect all variants that match the required env keys
                        let matching_variants: Vec<_> = detectors
                            .iter()
                            .filter_map(|d| {
                                let contrib = d.contribution(&manifest.dependencies);
                                if contrib.framework == fw.framework
                                    && env_keys
                                        .iter()
                                        .all(|k| contrib.runtime_env.contains_key(*k))
                                {
                                    Some(contrib)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if !matching_variants.is_empty() {
                            // Prefer the variant whose workdir matches the manifest's
                            // runtime workdir. This ensures UV projects (workdir /build)
                            // pick FlaskUv, while Poetry/PDM (workdir /app) pick their
                            // own variants.
                            let manifest_workdir = manifest.runtime_config.workdir.as_deref();
                            let best = matching_variants
                                .iter()
                                .find(|v| v.workdir.as_deref() == manifest_workdir)
                                .or(matching_variants.first());
                            if let Some(variant) = best {
                                framework = Some(variant.clone());
                            }
                        }
                    }
                }
            }

            results.push(ManifestWithFramework {
                path: file.path.clone(),
                manifest: *manifest.clone(),
                framework,
            });
        }
    }

    for child in &node.children {
        collect_manifests_with_frameworks(child, registry, results);
    }
}

// ── Step 3: Partition ───────────────────────────────────────────────────────

fn collect_configs(node: &DirNode) -> Vec<ConfigContribution> {
    let mut configs = Vec::new();
    for file in &node.files {
        if let FileKind::Config(config) = &file.kind {
            configs.push(config.clone());
        }
    }
    for child in &node.children {
        configs.extend(collect_configs(child));
    }
    configs
}

fn partition(
    tree: &RepoTree,
    manifests: Vec<ManifestWithFramework>,
    registry: &Registry,
) -> Vec<ServiceBucket> {
    // Group manifests by directory
    let mut dir_manifests: HashMap<PathBuf, Vec<ManifestWithFramework>> = HashMap::new();
    for mwf in manifests {
        let dir = mwf
            .path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        dir_manifests.entry(dir).or_default().push(mwf);
    }

    // Merge manifests in the same directory (handles Gradle case: settings.gradle + build.gradle)
    let mut merged: HashMap<PathBuf, ManifestWithFramework> = HashMap::new();
    for (dir, manifests_in_dir) in &mut dir_manifests {
        if manifests_in_dir.len() == 1 {
            merged.insert(dir.clone(), manifests_in_dir.remove(0));
        } else {
            // Sort by filename for deterministic merge order across platforms
            manifests_in_dir.sort_by(|a, b| a.path.cmp(&b.path));

            // Prefer lock file manifests (pnpm/yarn) over package.json
            let mut primary_idx = 0;
            let mut workspace_idx = None;
            let mut lock_file_idx = None;

            for (i, mwf) in manifests_in_dir.iter().enumerate() {
                if !mwf.manifest.build.commands.is_empty() {
                    primary_idx = i;
                }
                if mwf.manifest.workspace.is_some() {
                    workspace_idx = Some(i);
                }
                if registry
                    .get_profile(&mwf.manifest.build_system)
                    .is_some_and(|p| p.merge_priority)
                {
                    lock_file_idx = Some(i);
                }
            }

            // Lock files take priority over package.json
            if let Some(lf_idx) = lock_file_idx {
                primary_idx = lf_idx;
            }

            // Prefer manifests with dependencies (e.g., build.zig.zon over build.zig)
            // as they carry richer metadata (package name, specific artifacts)
            if lock_file_idx.is_none() {
                if let Some(dep_idx) = manifests_in_dir
                    .iter()
                    .position(|mwf| !mwf.manifest.dependencies.is_empty())
                {
                    primary_idx = dep_idx;
                }
            }

            let mut primary = manifests_in_dir.remove(primary_idx);

            // Merge workspace from the other manifest if the primary doesn't have one
            if primary.manifest.workspace.is_none() {
                if let Some(ws_idx) = workspace_idx {
                    let ws_idx = if ws_idx > primary_idx {
                        ws_idx - 1
                    } else {
                        ws_idx
                    };
                    if ws_idx < manifests_in_dir.len() {
                        primary.manifest.workspace =
                            manifests_in_dir[ws_idx].manifest.workspace.clone();
                    }
                }
            }

            // Merge package info: combine name and version from different manifests
            {
                let mut merged_name = primary.manifest.package.as_ref().and_then(|p| {
                    if p.name.is_empty() {
                        None
                    } else {
                        Some(p.name.clone())
                    }
                });
                let mut merged_version = primary
                    .manifest
                    .package
                    .as_ref()
                    .and_then(|p| p.version.clone());
                let mut merged_is_app = primary
                    .manifest
                    .package
                    .as_ref()
                    .map(|p| p.is_application)
                    .unwrap_or(false);

                for other in manifests_in_dir.iter() {
                    if let Some(other_pkg) = &other.manifest.package {
                        if merged_name.is_none() && !other_pkg.name.is_empty() {
                            merged_name = Some(other_pkg.name.clone());
                        }
                        if merged_version.is_none() {
                            merged_version.clone_from(&other_pkg.version);
                        }
                        if other_pkg.is_application {
                            merged_is_app = true;
                        }
                    }
                }

                if merged_name.is_some() || merged_version.is_some() {
                    primary.manifest.package = Some(Package {
                        name: merged_name.unwrap_or_default(),
                        version: merged_version,
                        is_application: merged_is_app,
                    });
                }
            }

            // Merge dependencies from other manifests (skip for lock file primaries
            // to avoid unintended framework detection)
            if lock_file_idx.is_none() {
                for other in manifests_in_dir.iter() {
                    for dep in &other.manifest.dependencies {
                        if !primary
                            .manifest
                            .dependencies
                            .iter()
                            .any(|d| d.name == dep.name)
                        {
                            primary.manifest.dependencies.push(dep.clone());
                        }
                    }
                }
            }

            // Merge build specs from secondary manifests of different languages
            // (e.g., PHP + Node.js in a Laravel + Vite project)
            for other in manifests_in_dir.iter() {
                if other.manifest.language != primary.manifest.language {
                    for pkg in &other.manifest.build.packages {
                        if !primary.manifest.build.packages.contains(pkg) {
                            primary.manifest.build.packages.push(pkg.clone());
                        }
                    }
                    primary
                        .manifest
                        .build
                        .commands
                        .extend(other.manifest.build.commands.clone());
                    for dir in &other.manifest.build.cache_dirs {
                        if !primary.manifest.build.cache_dirs.contains(dir) {
                            primary.manifest.build.cache_dirs.push(dir.clone());
                        }
                    }
                    primary
                        .manifest
                        .build
                        .env
                        .extend(other.manifest.build.env.clone());
                }
            }

            // Merge framework from other manifests (unless primary is a lock file).
            // Lock file primaries skip this because their dependency lists may differ
            // from package.json, and merging could introduce incorrect framework
            // contributions (e.g., health endpoints, runtime commands) that break
            // the container runtime. Instead, lock file primaries merge devDependency-
            // based frameworks separately (see below).
            if primary.framework.is_none() && lock_file_idx.is_none() {
                for other in manifests_in_dir.iter() {
                    if other.framework.is_some() {
                        primary.framework = other.framework.clone();
                        break;
                    }
                }
            }

            // For lock file primaries: merge framework ONLY from devDependency-based
            // detection. This covers cases like Vite, SvelteKit, Astro where the
            // framework package is a devDependency not included in lock file manifests.
            // We avoid merging production-dep frameworks (Fastify, Express) since
            // the lock file's own detection should handle those.
            if primary.framework.is_none() && lock_file_idx.is_some() {
                for other in manifests_in_dir.iter() {
                    if let Some(ref fw) = other.framework {
                        // Only merge if the framework was detected via devDependencies
                        let is_dev_framework = other
                            .manifest
                            .dependencies
                            .iter()
                            .filter(|d| d.scope == DepScope::Dev)
                            .any(|d| {
                                // Check if any dev dep name matches common build tool patterns
                                let n = d.name.as_str();
                                n == "vite"
                                    || n.starts_with("@sveltejs/")
                                    || n == "astro"
                                    || n.starts_with("@react-router/")
                                    || n.starts_with("@remix-run/")
                                    || n.starts_with("@solidjs/")
                                    || n.starts_with("@tanstack/")
                                    || n == "nuxt"
                                    || n == "nuxt3"
                            });
                        if is_dev_framework {
                            primary.framework = Some(fw.clone());
                            break;
                        }
                    }
                }
            }

            merged.insert(dir.clone(), primary);
        }
    }

    // Find workspace roots
    let workspace_roots: Vec<(PathBuf, Workspace)> = merged
        .iter()
        .filter_map(|(dir, mwf)| {
            mwf.manifest
                .workspace
                .as_ref()
                .map(|ws| (dir.clone(), ws.clone()))
        })
        .collect();

    // Collect all configs from the tree
    let all_configs = collect_configs(&tree.tree);

    // Group configs by directory
    let mut dir_configs: HashMap<PathBuf, Vec<ConfigContribution>> = HashMap::new();
    for config in all_configs {
        let dir = config
            .path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        dir_configs.entry(dir).or_default().push(config);
    }

    let mut buckets = Vec::new();

    // Check for turbo.json at repo root or workspace roots
    let has_turbo_json = tree
        .tree
        .files
        .iter()
        .any(|f| f.path.file_name().and_then(|n| n.to_str()) == Some("turbo.json"));

    if !workspace_roots.is_empty() {
        // Workspace mode: expand member patterns
        for (ws_root, workspace) in &workspace_roots {
            let expanded_members =
                expand_workspace_members(&tree.root, ws_root, &workspace.members, &merged);

            // Get workspace root manifest info for propagation
            let ws_root_manifest = merged.get(ws_root);
            let ws_root_build_env = ws_root_manifest.map(|m| m.manifest.build.env.clone());
            let ws_root_build_packages =
                ws_root_manifest.map(|m| m.manifest.build.packages.clone());
            let ws_root_runtime_env =
                ws_root_manifest.map(|m| m.manifest.runtime_config.env.clone());
            let ws_root_runtime_packages =
                ws_root_manifest.map(|m| m.manifest.runtime_config.packages.clone());
            // Extract build system info for workspace member propagation
            let ws_root_build_system = ws_root_manifest.map(|m| m.manifest.build_system);
            let ws_root_build_commands =
                ws_root_manifest.map(|m| m.manifest.build.commands.clone());
            let ws_root_cache_dirs = ws_root_manifest.map(|m| m.manifest.build.cache_dirs.clone());
            let ws_root_member_transform =
                ws_root_manifest.and_then(|m| m.manifest.build.member_transform.clone());
            let ws_root_artifacts = ws_root_manifest.map(|m| m.manifest.build.artifacts.clone());
            let ws_root_runtime_workdir =
                ws_root_manifest.and_then(|m| m.manifest.runtime_config.workdir.clone());

            for member_dir in expanded_members {
                if let Some(mut mwf) = merged.remove(&member_dir) {
                    let configs = collect_configs_for_service(&member_dir, ws_root, &dir_configs);
                    // Add .turbo to cache dirs when turbo.json is present
                    if has_turbo_json
                        && !mwf
                            .manifest
                            .build
                            .cache_dirs
                            .contains(&".turbo".to_string())
                    {
                        mwf.manifest.build.cache_dirs.insert(
                            1.min(mwf.manifest.build.cache_dirs.len()),
                            ".turbo".to_string(),
                        );
                    }

                    // Propagate build system from workspace root to members when
                    // the root's build system was determined by a lock file (merge_priority).
                    // This ensures workspace members use the correct package manager
                    // (e.g., yarn/pnpm instead of defaulting to npm, or UV instead of pip).
                    if let Some(ws_bs) = ws_root_build_system {
                        if registry
                            .get_profile(&ws_bs)
                            .is_some_and(|p| p.merge_priority)
                            && mwf.manifest.build_system != ws_bs
                        {
                            let old_pm = mwf.manifest.build_system.slug();
                            let new_pm = ws_bs.slug();

                            mwf.manifest.build_system = ws_bs;

                            // Replace build packages with root's (e.g., yarn instead of npm)
                            if let Some(ref pkgs) = ws_root_build_packages {
                                mwf.manifest.build.packages = pkgs.clone();
                            }

                            // Replace cache dirs with root's (e.g., .yarn-cache instead of .npm)
                            if let Some(ref cache) = ws_root_cache_dirs {
                                mwf.manifest.build.cache_dirs = cache.clone();
                            }

                            // Propagate member_transform, artifacts, and workdir from
                            // root when the member doesn't have its own member_transform.
                            // This handles cases like UV workspace members detected as
                            // plain pip projects that need the root's build commands,
                            // artifact paths (/build vs /app), and workdir.
                            if mwf.manifest.build.member_transform.is_none() {
                                if let Some(ref transform) = ws_root_member_transform {
                                    mwf.manifest.build.member_transform = Some(transform.clone());
                                }
                                if let Some(ref artifacts) = ws_root_artifacts {
                                    mwf.manifest.build.artifacts = artifacts.clone();
                                }
                                if let Some(ref workdir) = ws_root_runtime_workdir {
                                    mwf.manifest.runtime_config.workdir = Some(workdir.clone());
                                }
                            }

                            // Replace install command with root's, and update package manager
                            // name in build commands
                            if let Some(ref root_cmds) = ws_root_build_commands {
                                if !root_cmds.is_empty() {
                                    if !mwf.manifest.build.commands.is_empty() {
                                        mwf.manifest.build.commands[0] = root_cmds[0].clone();
                                    }
                                    for cmd in mwf.manifest.build.commands.iter_mut().skip(1) {
                                        *cmd = cmd.replace(old_pm, new_pm);
                                    }
                                }
                            }

                            // Ensure the new package manager is in runtime packages
                            // since the entrypoint override will use it (e.g., "yarn start")
                            let new_pm_pkg = new_pm.to_string();
                            if !mwf.manifest.runtime_config.packages.contains(&new_pm_pkg) {
                                mwf.manifest.runtime_config.packages.push(new_pm_pkg);
                            }

                            // Update member_transform commands similarly
                            if let Some(ref mut transform) = mwf.manifest.build.member_transform {
                                if let Some(ref root_cmds) = ws_root_build_commands {
                                    if !root_cmds.is_empty()
                                        && !transform.member_commands.is_empty()
                                    {
                                        transform.member_commands[0] = root_cmds[0].clone();
                                        for cmd in transform.member_commands.iter_mut().skip(1) {
                                            *cmd = cmd.replace(old_pm, new_pm);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Propagate versioned packages from workspace root to members
                    // (e.g., openjdk-17 from parent pom.xml to child modules)
                    if let Some(ref ws_packages) = ws_root_build_packages {
                        propagate_versioned_packages(&mut mwf.manifest.build.packages, ws_packages);
                    }
                    if let Some(ref ws_packages) = ws_root_runtime_packages {
                        propagate_versioned_packages(
                            &mut mwf.manifest.runtime_config.packages,
                            ws_packages,
                        );
                    }
                    // Propagate build env from workspace root (e.g., JAVA_HOME).
                    // Use insert (overwrite) because child modules may have default values
                    // (e.g., JAVA_HOME=java-21) that should be replaced by the workspace root's
                    // specific version (e.g., JAVA_HOME=java-17).
                    if let Some(ref ws_env) = ws_root_build_env {
                        for (k, v) in ws_env {
                            mwf.manifest.build.env.insert(k.clone(), v.clone());
                        }
                    }
                    if let Some(ref ws_env) = ws_root_runtime_env {
                        for (k, v) in ws_env {
                            mwf.manifest.runtime_config.env.insert(k.clone(), v.clone());
                        }
                    }

                    // For workspace members that are libraries (no start/build scripts,
                    // no framework detected), clear any main-field-derived entrypoint
                    // to prevent false-positive service detection.
                    let is_app = mwf
                        .manifest
                        .package
                        .as_ref()
                        .map(|p| p.is_application)
                        .unwrap_or(false);
                    if !is_app && mwf.framework.is_none() {
                        mwf.manifest.runtime_config.entrypoint = None;
                    }

                    buckets.push(ServiceBucket {
                        path: member_dir,
                        manifest: mwf.manifest,
                        configs,
                        framework: mwf.framework,
                        is_workspace_member: true,
                        workspace_root: Some(ws_root.clone()),
                    });
                }
            }

            // Remove workspace root from merged so it doesn't become a standalone project,
            // UNLESS the root itself is an application AND no workspace members are
            // applications. This handles pnpm workspaces where the root has scripts.start
            // and workspace packages are internal libraries (not deployable services).
            let any_member_is_app = buckets
                .iter()
                .filter(|b| b.workspace_root.as_ref() == Some(ws_root))
                .any(|b| {
                    b.manifest
                        .package
                        .as_ref()
                        .map(|p| p.is_application)
                        .unwrap_or(false)
                        || b.framework.is_some()
                });

            if !any_member_is_app {
                if let Some(ws_mwf) = merged.get(ws_root) {
                    let root_is_app = ws_mwf
                        .manifest
                        .package
                        .as_ref()
                        .map(|p| p.is_application)
                        .unwrap_or(false);
                    let root_has_framework = ws_mwf.framework.is_some();
                    let root_has_entrypoint = ws_mwf.manifest.runtime_config.entrypoint.is_some();

                    if root_is_app || root_has_framework || root_has_entrypoint {
                        // Keep the workspace root as a standalone project — it's the only
                        // deployable service (e.g., pnpm workspace with library packages)
                    } else {
                        merged.remove(ws_root);
                    }
                } else {
                    merged.remove(ws_root);
                }
            } else {
                // Workspace has application members — the root is just a coordinator
                merged.remove(ws_root);
            }
        }
    }

    // Any remaining manifests are standalone projects
    for (dir, mwf) in merged {
        // Skip if this dir was already consumed by a workspace
        let configs = dir_configs.remove(&dir).unwrap_or_default();

        // Also grab root-level configs if this is a root project
        let mut all_service_configs = configs;
        if dir.as_os_str().is_empty() || dir == Path::new(".") {
            if let Some(root_configs) = dir_configs.remove(Path::new("")) {
                all_service_configs.extend(root_configs);
            }
        }

        buckets.push(ServiceBucket {
            path: dir,
            manifest: mwf.manifest,
            configs: all_service_configs,
            framework: mwf.framework,
            is_workspace_member: false,
            workspace_root: None,
        });
    }

    buckets
}

/// Propagate versioned packages from workspace root to member.
/// If the member has an unversioned package (e.g., "openjdk") and the root has a versioned
/// one (e.g., "openjdk-17"), replace the member's with the root's version.
fn propagate_versioned_packages(member_packages: &mut [String], root_packages: &[String]) {
    for member_pkg in member_packages.iter_mut() {
        // Skip already-versioned packages
        if member_pkg.contains('-') {
            continue;
        }
        // Look for a versioned variant in root packages
        let prefix = format!("{}-", member_pkg);
        if let Some(root_pkg) = root_packages.iter().find(|p| p.starts_with(&prefix)) {
            *member_pkg = root_pkg.clone();
        }
    }
}

fn expand_workspace_members(
    repo_root: &Path,
    ws_root: &Path,
    patterns: &[String],
    merged: &HashMap<PathBuf, ManifestWithFramework>,
) -> Vec<PathBuf> {
    let mut members = Vec::new();
    let abs_ws_root = if ws_root.as_os_str().is_empty() {
        repo_root.to_path_buf()
    } else {
        repo_root.join(ws_root)
    };

    for pattern in patterns {
        if pattern.contains('*') {
            // Glob expansion
            let glob_pattern = abs_ws_root.join(pattern);
            if let Ok(entries) = glob::glob(&glob_pattern.to_string_lossy()) {
                for entry in entries.flatten() {
                    if entry.is_dir() {
                        if let Ok(rel) = entry.strip_prefix(repo_root) {
                            members.push(rel.to_path_buf());
                        }
                    }
                }
            } else {
                // Fallback: match against known manifest directories
                let base = pattern.trim_end_matches("/*").trim_end_matches("/*");
                for dir in merged.keys() {
                    let dir_str = dir.to_string_lossy();
                    if dir_str.starts_with(base) && dir != ws_root {
                        members.push(dir.clone());
                    }
                }
            }
        } else {
            // Exact path
            let member_path = if ws_root.as_os_str().is_empty() {
                PathBuf::from(pattern.trim_start_matches(':'))
            } else {
                ws_root.join(pattern.trim_start_matches(':'))
            };
            members.push(member_path);
        }
    }

    members
}

fn collect_configs_for_service(
    service_dir: &Path,
    workspace_root: &Path,
    dir_configs: &HashMap<PathBuf, Vec<ConfigContribution>>,
) -> Vec<ConfigContribution> {
    let mut configs = Vec::new();

    // Get configs from the service's own directory
    if let Some(service_configs) = dir_configs.get(service_dir) {
        configs.extend(service_configs.clone());
    }

    // Get configs from workspace root (shared configs)
    if service_dir != workspace_root {
        if let Some(root_configs) = dir_configs.get(workspace_root) {
            configs.extend(root_configs.clone());
        }
    }

    // Get configs from repo root
    let root = Path::new("");
    if service_dir != root && workspace_root != root {
        if let Some(root_configs) = dir_configs.get(root) {
            configs.extend(root_configs.clone());
        }
    }

    configs
}

// ── Step 4: Reduce ──────────────────────────────────────────────────────────

fn reduce(bucket: ServiceBucket, registry: &Registry) -> Result<UniversalBuild> {
    let m = &bucket.manifest;
    let profile = registry.get_profile(&m.build_system);

    // Check if this is a standalone project in a subdirectory
    let is_subdirectory = !bucket.is_workspace_member
        && !bucket.path.as_os_str().is_empty()
        && bucket.path != Path::new(".");
    let subdir = bucket.path.to_string_lossy().to_string();

    // Resolve build commands (workspace-aware or subdirectory-aware)
    let build_commands = if bucket.is_workspace_member {
        if let Some(transform) = &m.build.member_transform {
            transform
                .member_commands
                .iter()
                .map(|cmd| {
                    cmd.replace("{module}", &bucket.module_name())
                        .replace("{package}", &bucket.package_name())
                        .replace("{root}", &bucket.workspace_root_display())
                })
                .collect()
        } else {
            m.build.commands.clone()
        }
    } else if is_subdirectory {
        // For standalone projects in subdirectories, delegate to build system profile
        let transform = profile
            .map(|p| p.transform_subdirectory_command)
            .unwrap_or(default_subdirectory_command);
        m.build
            .commands
            .iter()
            .map(|cmd| transform(cmd, &subdir))
            .collect()
    } else {
        m.build.commands.clone()
    };

    // Resolve artifacts (workspace-aware or subdirectory-aware)
    let mut artifacts: Vec<CopySpec> = if bucket.is_workspace_member {
        if let Some(transform) = &m.build.member_transform {
            if let Some(member_artifacts) = &transform.member_artifacts {
                member_artifacts
                    .iter()
                    .map(|(from, to)| CopySpec {
                        from: from
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                        to: to
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                    })
                    .collect()
            } else {
                m.build
                    .artifacts
                    .iter()
                    .map(|(from, to)| CopySpec {
                        from: from
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                        to: to
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                    })
                    .collect()
            }
        } else {
            m.build
                .artifacts
                .iter()
                .map(|(from, to)| CopySpec {
                    from: from.clone(),
                    to: to.clone(),
                })
                .collect()
        }
    } else if is_subdirectory {
        // For standalone subdirectory projects, prepend directory to artifact paths
        // Exception: build systems with shared_target_dir (e.g., Cargo uses --target-dir target)
        let uses_shared_target = profile.is_some_and(|p| p.shared_target_dir);
        m.build
            .artifacts
            .iter()
            .map(|(from, to)| CopySpec {
                from: if from.starts_with('/') || from.starts_with('.') || uses_shared_target {
                    from.clone()
                } else {
                    format!("{}/{}", subdir, from)
                },
                to: to.clone(),
            })
            .collect()
    } else {
        m.build
            .artifacts
            .iter()
            .map(|(from, to)| CopySpec {
                from: from.clone(),
                to: to.clone(),
            })
            .collect()
    };

    // Delegate artifact post-processing to build system profile (e.g., Gradle JAR name resolution)
    if let Some(p) = profile {
        (p.resolve_artifacts)(&mut artifacts, m.package.as_ref());
    }

    // Merge config contributions into runtime spec
    let mut runtime_env = m.runtime_config.env.clone();
    let mut runtime_ports = m.runtime_config.ports.clone();
    let mut runtime_packages = m.runtime_config.packages.clone();
    let mut health_endpoint = m.runtime_config.health_endpoint.clone();
    let mut config_runtime_command: Option<String> = None;

    for config in &bucket.configs {
        runtime_env.extend(config.env_vars.clone());
        runtime_ports.extend(config.ports.clone());
        if health_endpoint.is_none() {
            health_endpoint.clone_from(&config.health_endpoint);
        }
        if config_runtime_command.is_none() {
            config_runtime_command.clone_from(&config.runtime_command);
        }
    }

    // Apply framework contribution
    let mut framework_runtime_command = None;
    let mut framework_workdir = None;
    let framework_name = if let Some(fw) = &bucket.framework {
        runtime_env.extend(fw.env_vars.clone());
        runtime_env.extend(fw.runtime_env.clone());
        runtime_packages.extend(fw.runtime_packages.clone());
        // Framework ports always override when specified
        if !fw.default_ports.is_empty() {
            runtime_ports = fw.default_ports.clone();
        }
        if health_endpoint.is_none() {
            health_endpoint = fw.health_endpoints.first().cloned();
        }
        framework_runtime_command = fw.runtime_command.clone();
        framework_workdir = fw.workdir.clone();

        // Framework extra_copy replaces generic artifacts when non-empty
        if !fw.extra_copy.is_empty() {
            artifacts = fw
                .extra_copy
                .iter()
                .map(|(from, to)| CopySpec {
                    from: from.clone(),
                    to: to.clone(),
                })
                .collect();
        }
        Some(fw.framework.name())
    } else {
        None
    };

    // When a config provides a runtime command (e.g., Procfile), its ports should
    // take priority over framework defaults since it represents an explicit user declaration.
    // Insert config ports at the front so they are used for health checks (which use the first port).
    if config_runtime_command.is_some() {
        let mut config_ports = Vec::new();
        for config in &bucket.configs {
            if config.runtime_command.is_some() {
                for &port in &config.ports {
                    if !config_ports.contains(&port) {
                        config_ports.push(port);
                    }
                }
            }
        }
        // Remove any config ports already in runtime_ports, then prepend them
        runtime_ports.retain(|p| !config_ports.contains(p));
        config_ports.extend(runtime_ports);
        runtime_ports = config_ports;
    }

    // When framework sets workdir, update artifact copy targets from /app to framework workdir
    if let Some(ref fw_workdir) = framework_workdir {
        if fw_workdir != "/app" {
            for artifact in &mut artifacts {
                if artifact.to == "/app" || artifact.to == "/app/" {
                    artifact.to = fw_workdir.clone();
                }
            }
        }
    }

    // Deduplicate (preserve insertion order, remove later duplicates)
    {
        let mut seen = std::collections::HashSet::new();
        runtime_ports.retain(|p| seen.insert(*p));
    }
    {
        let mut seen = std::collections::HashSet::new();
        runtime_packages.retain(|p| seen.insert(p.clone()));
    }

    // Determine project name
    let is_root_project = bucket.path.as_os_str().is_empty() || bucket.path == Path::new(".");
    let project_name = if is_root_project {
        // For root-level projects: use package name only from strong naming sources,
        // unless the build system profile opts out (e.g., Gradle, Poetry, Pip)
        if profile.is_some_and(|p| !p.use_package_name_for_root) {
            Some("app".into())
        } else {
            m.package
                .as_ref()
                .filter(|p| !p.name.is_empty())
                .map(|p| p.name.clone())
                .or(Some("app".into()))
        }
    } else {
        // Non-root: package name or directory name
        m.package
            .as_ref()
            .filter(|p| !p.name.is_empty())
            .map(|p| p.name.clone())
            .or_else(|| {
                bucket
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
    };

    // Build entrypoint command: config (Procfile) > framework override > manifest entrypoint
    let mut entrypoint_cmd = if let Some(config_cmd) = config_runtime_command {
        // Extract leading KEY=VALUE tokens as environment variables so they don't
        // end up as the executable in the command array.
        let parts: Vec<&str> = config_cmd.split_whitespace().collect();
        let env_prefix_end = parts
            .iter()
            .position(|p| !p.contains('=') || p.starts_with('/') || p.starts_with('.'))
            .unwrap_or(parts.len());
        for kv in &parts[..env_prefix_end] {
            if let Some((k, v)) = kv.split_once('=') {
                runtime_env
                    .entry(k.to_string())
                    .or_insert_with(|| v.to_string());
            }
        }
        let remaining: &str = &config_cmd[config_cmd
            .find(parts.get(env_prefix_end).copied().unwrap_or(""))
            .unwrap_or(0)..];
        let remaining = remaining.trim();
        // If the command contains shell operators, wrap with sh -c to preserve semantics.
        let has_shell_ops = remaining.contains("&&")
            || remaining.contains("||")
            || remaining.contains('|')
            || remaining.contains(';')
            || remaining.contains("$(")
            || remaining.contains("${");
        if has_shell_ops {
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                remaining.to_string(),
            ]
        } else {
            remaining.split_whitespace().map(String::from).collect()
        }
    } else if let Some(fw_cmd) = framework_runtime_command {
        fw_cmd
    } else if let Some(entrypoint) = &m.runtime_config.entrypoint {
        // For non-root projects, build system profile may override the entrypoint
        // (e.g., Node.js uses `npm start` instead of direct command).
        // Only apply the override when the package has a start script (is_application),
        // not when the entrypoint was derived from the `main` field.
        let has_start_script = m
            .package
            .as_ref()
            .map(|p| p.is_application)
            .unwrap_or(false);
        if !is_root_project && has_start_script {
            if let Some(override_cmd) = profile.and_then(|p| p.non_root_entrypoint_override) {
                override_cmd.iter().map(|s| s.to_string()).collect()
            } else {
                entrypoint.split_whitespace().map(String::from).collect()
            }
        } else {
            entrypoint.split_whitespace().map(String::from).collect()
        }
    } else {
        vec![]
    };

    // For Gradle installDist projects, resolve the generic `/app/bin/app` entrypoint
    // to `/app/bin/{project_name}` using the actual project name from settings.gradle.
    // installDist creates scripts named after the project, not "app".
    if m.build_system.slug() == "gradle" {
        if let Some(pkg) = &m.package {
            if !pkg.name.is_empty() && pkg.name != "app" {
                for part in entrypoint_cmd.iter_mut() {
                    if *part == "/app/bin/app" {
                        *part = format!("/app/bin/{}", pkg.name);
                    }
                }
            }
        }
    }

    // For Ruby/Bundler projects, ensure `bundle exec` wraps the entrypoint
    // so gems installed in vendor/bundle are on the load path.
    if m.build_system.slug() == "bundler"
        && !entrypoint_cmd.is_empty()
        && entrypoint_cmd.first().map(|s| s.as_str()) != Some("bundle")
        && entrypoint_cmd
            .iter()
            .any(|s| s == "ruby" || s.ends_with(".rb"))
    {
        let mut wrapped = vec!["bundle".to_string(), "exec".to_string()];
        wrapped.extend(entrypoint_cmd);
        entrypoint_cmd = wrapped;
    }

    // For polyglot projects where the entrypoint uses a language not in the primary
    // runtime packages: add the required runtime binaries (e.g., Node+Ruby project
    // where Procfile runs `ruby app.rb` but primary language is JavaScript).
    let cmd_uses_ruby = entrypoint_cmd
        .iter()
        .any(|s| s == "ruby" || s == "bundle" || s.ends_with(".rb"));
    if cmd_uses_ruby && !runtime_packages.iter().any(|p| p.starts_with("ruby")) {
        // Find ruby packages from build packages
        for pkg in &m.build.packages {
            if ((pkg.starts_with("ruby") && !pkg.contains("-dev")) || pkg == "bundler")
                && !runtime_packages.contains(pkg)
            {
                runtime_packages.push(pkg.clone());
            }
        }
    }

    // Workdir: framework override > manifest workdir
    // For workspace members with adjusts_workspace_member_workdir, set workdir to the
    // member's directory so that the entrypoint command runs in the correct context
    let workdir = if bucket.is_workspace_member
        && profile.is_some_and(|p| p.adjusts_workspace_member_workdir)
    {
        let base = framework_workdir
            .or_else(|| m.runtime_config.workdir.clone())
            .unwrap_or_else(|| "/app".into());
        let member_path = bucket.path.display().to_string();
        if member_path.is_empty() || member_path == "." {
            base
        } else {
            format!("{}/{}", base, member_path)
        }
    } else {
        framework_workdir
            .or_else(|| m.runtime_config.workdir.clone())
            .unwrap_or_else(|| "/app".into())
    };

    Ok(UniversalBuild {
        version: "1.0".into(),
        metadata: BuildMetadata {
            project_name,
            language: m.language.name(),
            build_system: m.build_system.name(),
            framework: framework_name,
            reasoning: {
                let filename = m
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let parent = m.path.parent().filter(|p| !p.as_os_str().is_empty());
                match parent {
                    Some(dir) => format!("Detected from {} in {}", filename, dir.display()),
                    None => format!("Detected from {}", filename),
                }
            },
        },
        build: BuildStage {
            packages: m.build.packages.clone(),
            env: m.build.env.clone(),
            commands: build_commands,
            cache: m.build.cache_dirs.clone(),
            build_image: m.build.build_image.clone(),
        },
        runtime: RuntimeStage {
            packages: runtime_packages,
            env: runtime_env,
            copy: artifacts,
            command: entrypoint_cmd,
            workdir,
            ports: runtime_ports,
            health: health_endpoint.map(|e| HealthCheck { endpoint: e }),
        },
    })
}

// ── Step 5: Wolfi Package Resolution ─────────────────────────────────────

/// Known package prefixes that need version resolution.
/// Maps the generic name to the prefix used for Wolfi lookup.
const VERSIONABLE_PACKAGES: &[(&str, &str)] = &[
    ("rust", "rust"),
    ("nodejs", "nodejs"),
    ("python", "python"),
    ("openjdk", "openjdk"),
    ("go", "go"),
    ("ruby", "ruby"),
    ("php", "php"),
    ("elixir", "elixir"),
    ("erlang", "erlang"),
    ("dotnet-sdk", "dotnet"),
    ("dotnet-runtime", "dotnet"),
    ("maven", "maven"),
    ("gradle", "gradle"),
    ("zig", "zig"),
    ("bazel", "bazel"),
    ("postgresql", "postgresql"),
    ("libpq", "libpq"),
];

/// Packages where the very latest version often lacks broad ecosystem
/// compatibility (many libraries don't publish wheels/packages for the
/// bleeding-edge release). For these, prefer the second-latest minor version
/// when no explicit version is pinned — matching PaaS defaults (Heroku, Railway).
const PREFER_STABLE_PACKAGES: &[(&str, usize)] = &[
    ("python", 2), // Many libraries publish wheels late; N-2 has broadest support
    ("elixir", 1),
    ("erlang", 2), // Erlang 27+ has escript compilation issues with popular packages (e.g. simplifile)
];

/// Resolve generic package names to versioned Wolfi package names.
fn resolve_wolfi_packages(packages: &mut [String], wolfi: &WolfiPackageIndex) {
    for pkg in packages.iter_mut() {
        // Skip if already exists in Wolfi (versioned or generic like "build-base")
        if wolfi.has_package(pkg) {
            continue;
        }

        // Handle special cases first
        if pkg == "pip" {
            // pip → py3.X-pip (derive from python version in same package list)
            // We'll handle this in a second pass
            continue;
        }

        // Check if this is a versionable package
        if let Some((_, prefix)) = VERSIONABLE_PACKAGES
            .iter()
            .find(|(name, _)| *name == pkg.as_str())
        {
            let resolved = if let Some((_, offset)) =
                PREFER_STABLE_PACKAGES.iter().find(|(p, _)| *p == *prefix)
            {
                wolfi.get_stable_version_at_offset(prefix, *offset)
            } else {
                wolfi.get_latest_version(prefix)
            };
            if let Some(resolved) = resolved {
                debug!(from = %pkg, to = %resolved, "Resolved Wolfi package version");
                *pkg = resolved;
            }
        } else if pkg.ends_with("-dev") {
            // Handle e.g. erlang-dev → erlang-28-dev
            let base = pkg.strip_suffix("-dev").unwrap();
            if let Some((_, prefix)) = VERSIONABLE_PACKAGES.iter().find(|(name, _)| *name == base) {
                let resolved = if let Some((_, offset)) =
                    PREFER_STABLE_PACKAGES.iter().find(|(p, _)| *p == *prefix)
                {
                    wolfi.get_stable_version_at_offset(prefix, *offset)
                } else {
                    wolfi.get_latest_version(prefix)
                };
                if let Some(resolved) = resolved {
                    let dev_pkg = format!("{}-dev", resolved);
                    if wolfi.has_package(&dev_pkg) {
                        debug!(from = %pkg, to = %dev_pkg, "Resolved Wolfi dev package version");
                        *pkg = dev_pkg;
                    }
                }
            } else if base.starts_with("ruby-") {
                // ruby-2.6-dev → ruby-3.0-dev (EOL version fallback)
                let versions = wolfi.get_versions("ruby");
                if let Some(oldest) = versions.last() {
                    let resolved = format!("ruby-{}-dev", oldest);
                    debug!(from = %pkg, to = %resolved, "Resolved unavailable Ruby -dev version to oldest Wolfi package");
                    *pkg = resolved;
                }
            }
        } else if pkg.starts_with("openjdk-") && !pkg.contains("-jre") {
            // openjdk-17 → check it exists, if not try with wolfi
            if !wolfi.has_package(pkg) {
                // Try finding the exact version
                let version = pkg.strip_prefix("openjdk-").unwrap_or("");
                let available = wolfi.get_versions("openjdk");
                if let Some(resolved) = wolfi.match_version("openjdk", version, &available) {
                    *pkg = resolved;
                }
            }
        } else if pkg.starts_with("dotnet-") && pkg.ends_with("-sdk") {
            // dotnet-8-sdk → already versioned, check existence
            if !wolfi.has_package(pkg) {
                // Try resolving: dotnet-sdk → dotnet-X-sdk
                if let Some(latest) = wolfi.get_latest_version("dotnet") {
                    let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                    *pkg = format!("dotnet-{}-sdk", ver);
                }
            }
        } else if pkg.starts_with("dotnet-") && pkg.ends_with("-runtime") && !wolfi.has_package(pkg)
        {
            if let Some(latest) = wolfi.get_latest_version("dotnet") {
                let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                *pkg = format!("dotnet-{}-runtime", ver);
            }
        } else if pkg.starts_with("aspnet-") && pkg.ends_with("-runtime") && !wolfi.has_package(pkg)
        {
            // aspnet-6-runtime → aspnet-X-runtime (resolve to latest available)
            if let Some(latest) = wolfi.get_latest_version("dotnet") {
                let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                *pkg = format!("aspnet-{}-runtime", ver);
            }
        } else if pkg.starts_with("go-") && !wolfi.has_package(pkg) {
            // go-1.18 → resolve to latest available Go version
            // Go is backward-compatible, so old code builds fine with newer compilers.
            if let Some(latest) = wolfi.get_latest_version("go") {
                debug!(from = %pkg, to = %latest, "Resolved old Go version to latest Wolfi package");
                *pkg = latest;
            }
        } else if pkg.starts_with("python-")
            && pkg[7..].chars().next().is_some_and(|c| c.is_ascii_digit())
            && !wolfi.has_package(pkg)
        {
            // python-2.7 → resolve to preferred stable Python version
            // Python 2 isn't in Wolfi; fall back to the stable Python 3.x
            // (N-2 offset matching PREFER_STABLE_PACKAGES for broadest support).
            if let Some(resolved) = wolfi.get_stable_version_at_offset("python", 2) {
                debug!(from = %pkg, to = %resolved, "Resolved unavailable Python version to stable Wolfi package");
                *pkg = resolved;
            }
        } else if pkg.starts_with("ruby-")
            && !pkg.ends_with("-dev")
            && pkg[5..].chars().next().is_some_and(|c| c.is_ascii_digit())
            && !wolfi.has_package(pkg)
        {
            // ruby-2.6 → resolve to minimum available Ruby version
            // EOL Ruby versions aren't in Wolfi; fall back to oldest available.
            let versions = wolfi.get_versions("ruby");
            if let Some(oldest) = versions.last() {
                let resolved = format!("ruby-{}", oldest);
                debug!(from = %pkg, to = %resolved, "Resolved unavailable Ruby version to oldest Wolfi package");
                *pkg = resolved;
            }
        }
    }

    // Second pass: resolve pip based on python version
    let python_version = packages
        .iter()
        .find(|p| p.starts_with("python-"))
        .and_then(|p| p.strip_prefix("python-"))
        .map(String::from);

    if let Some(py_ver) = python_version {
        for pkg in packages.iter_mut() {
            if pkg == "pip" {
                let pip_pkg = format!("py{}-pip", py_ver);
                if wolfi.has_package(&pip_pkg) {
                    *pkg = pip_pkg;
                } else {
                    // Try just the major.minor
                    let short_ver = py_ver.split('.').take(2).collect::<Vec<_>>().join(".");
                    let pip_pkg = format!("py{}-pip", short_ver);
                    if wolfi.has_package(&pip_pkg) {
                        *pkg = pip_pkg;
                    }
                }
            }
        }
    }

    // Third pass: resolve Ruby-related packages with version context
    // ruby is already resolved to e.g. ruby-3.4 by VERSIONABLE_PACKAGES
    let ruby_version = packages
        .iter()
        .find(|p| {
            p.starts_with("ruby-") && p[5..].chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .and_then(|p| p.strip_prefix("ruby-"))
        .map(String::from);

    if let Some(rb_ver) = ruby_version {
        for pkg in packages.iter_mut() {
            if pkg == "bundler" || pkg == "ruby-bundler" {
                // ruby-bundler → ruby3.4-bundler (note: no dash between ruby and version)
                let bundler_pkg = format!("ruby{}-bundler", rb_ver);
                if wolfi.has_package(&bundler_pkg) {
                    *pkg = bundler_pkg;
                }
            } else if pkg == "ruby-dev" {
                // ruby-dev → ruby-3.4-dev
                let dev_pkg = format!("ruby-{}-dev", rb_ver);
                if wolfi.has_package(&dev_pkg) {
                    *pkg = dev_pkg;
                }
            }
        }
    }
}

// ── Version file scanning ────────────────────────────────────────────────

/// Scan for language version files (.nvmrc, .node-version, .python-version)
/// and update package names to include specific versions.
fn scan_version_files(repo_root: &Path, build: &mut UniversalBuild) {
    let language = &build.metadata.language;
    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    match language.as_str() {
        "JavaScript" | "TypeScript" => {
            if let Some(version) = read_node_version(&project_dir, repo_root) {
                let versioned_pkg = format!("nodejs-{}", version);
                replace_package(&mut build.build.packages, "nodejs", &versioned_pkg);
                replace_package(&mut build.runtime.packages, "nodejs", &versioned_pkg);
            }
        }
        "Python" => {
            if let Some(version) = read_python_version(&project_dir, repo_root) {
                let versioned_pkg = format!("python-{}", version);
                replace_package(&mut build.build.packages, "python", &versioned_pkg);
                replace_package(&mut build.runtime.packages, "python", &versioned_pkg);
            }
        }
        "PHP" => {
            if let Some(version) = read_php_version(&project_dir, repo_root) {
                let versioned_pkg = format!("php-{}", version);
                replace_package(&mut build.build.packages, "php", &versioned_pkg);
                replace_package(&mut build.runtime.packages, "php", &versioned_pkg);
            }
        }
        "Ruby" => {
            if let Some(version) = read_ruby_version(&project_dir, repo_root) {
                let versioned_pkg = format!("ruby-{}", version);
                let versioned_dev = format!("ruby-{}-dev", version);
                replace_package(&mut build.build.packages, "ruby", &versioned_pkg);
                replace_package(&mut build.build.packages, "ruby-dev", &versioned_dev);
                replace_package(&mut build.runtime.packages, "ruby", &versioned_pkg);
            }
        }
        "Rust" => {
            // Only build packages need the rust compiler; runtime uses the compiled binary
            if let Some(version) = crate::version::rust::read_rust_version(&project_dir, repo_root)
            {
                let versioned_pkg = format!("rust-{}", version);
                replace_package(&mut build.build.packages, "rust", &versioned_pkg);
            }
        }
        "Swift" => {
            // Pin the Swift Docker build image to a specific version from .swift-version.
            if let Some(version) = read_swift_version(&project_dir, repo_root) {
                build.build.build_image = Some(format!("docker.io/library/swift:{}", version));
            }
        }
        _ => {}
    }
}

// ── Framework fallback entrypoints ───────────────────────────────────────

/// For detected frameworks without an explicit start script, provide a
/// production-ready fallback entrypoint command. This only triggers when
/// the build has a detected framework but no runtime command (i.e., no
/// scripts.start, no main field, no Procfile).
fn provide_framework_fallback_entrypoint(build: &mut UniversalBuild) {
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

// ── Yarn Berry corepack entrypoint wrapping ─────────────────────────────

/// For Yarn >= 2 (Berry) projects, the runtime entrypoint needs `corepack enable`
/// before `yarn start` because the Wolfi `yarn` package provides Yarn 1.x.
/// Corepack reads the `packageManager` field from package.json and uses the
/// correct Yarn version.
fn wrap_yarn_corepack_entrypoint(build: &mut UniversalBuild) {
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

// ── Python entrypoint scanning ───────────────────────────────────────────

/// Common Python entrypoint filenames, ordered by priority.
const PYTHON_ENTRYPOINTS: &[&str] = &["app.py", "main.py", "server.py", "wsgi.py", "manage.py"];

/// Scan project directory for common Python entrypoint files.
/// Only overrides if current entrypoint is the fallback "python -m {name}".
fn scan_python_entrypoints(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    // Only override fallback entrypoints
    let is_fallback = build.runtime.command.is_empty()
        || (build.runtime.command.len() == 3
            && build.runtime.command[0] == "python"
            && build.runtime.command[1] == "-m");

    if !is_fallback {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    if !project_dir.is_dir() {
        return;
    }

    for filename in PYTHON_ENTRYPOINTS {
        if project_dir.join(filename).exists() {
            let workdir = &build.runtime.workdir;
            let entrypoint_path = format!("{}/{}", workdir, filename);
            debug!(entrypoint = %entrypoint_path, "Found Python entrypoint");
            build.runtime.command = vec!["python".into(), entrypoint_path];
            return;
        }
    }
}

// ── Python native dependency detection ──────────────────────────────────

/// Maps well-known Python packages to their required system build/runtime packages.
/// Each entry: (python_package_pattern, build_packages, runtime_packages).
const PYTHON_NATIVE_DEPS: &[(&str, &[&str], &[&str])] = &[
    // PostgreSQL adapters (C compilation needs headers; binary variant also needs headers
    // as a fallback when no pre-built wheel is available for the target Python version)
    ("psycopg2", &["postgresql-dev"], &["libpq"]),
    ("psycopg2-binary", &["postgresql-dev"], &["libpq"]),
    // psycopg[c] or psycopg with C backend
    ("psycopg", &["postgresql-dev"], &["libpq"]),
    // MySQL adapter (needs both -dev for headers/symlinks and base for libmariadb.so.3)
    (
        "mysqlclient",
        &["mariadb-connector-c-dev", "mariadb-connector-c"],
        &["mariadb-connector-c"],
    ),
    // Cairo graphics
    ("pycairo", &["cairo-dev"], &["cairo"]),
    // PDF processing (poppler)
    ("pdf2image", &["poppler-utils"], &["poppler-utils"]),
    // Audio processing
    ("pydub", &["ffmpeg"], &["ffmpeg"]),
    // Pillow (image processing)
    (
        "Pillow",
        &["freetype-dev", "libjpeg-turbo-dev", "zlib-dev"],
        &["freetype", "libjpeg-turbo", "zlib"],
    ),
    (
        "pillow",
        &["freetype-dev", "libjpeg-turbo-dev", "zlib-dev"],
        &["freetype", "libjpeg-turbo", "zlib"],
    ),
    // Cryptography
    (
        "cryptography",
        &["openssl-dev", "libffi-dev"],
        &["openssl", "libffi"],
    ),
    // lxml
    (
        "lxml",
        &["libxml2-dev", "libxslt-dev"],
        &["libxml2", "libxslt"],
    ),
    // cffi
    ("cffi", &["libffi-dev"], &["libffi"]),
];

/// Detect Django DJANGO_SETTINGS_MODULE from manage.py.
fn fix_django_settings(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }
    if build.metadata.framework.as_deref() != Some("Django") {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let manage_py = project_dir.join("manage.py");
    if !manage_py.exists() {
        return;
    }

    if let Ok(content) = std::fs::read_to_string(&manage_py) {
        // Look for: os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'some.module')
        // or: os.environ.setdefault("DJANGO_SETTINGS_MODULE", "some.module")
        let re = regex::Regex::new(r#"DJANGO_SETTINGS_MODULE['"]\s*,\s*['"]([\w.]+)['"]"#).unwrap();
        if let Some(caps) = re.captures(&content) {
            let settings_module = caps.get(1).unwrap().as_str();
            build
                .runtime
                .env
                .insert("DJANGO_SETTINGS_MODULE".into(), settings_module.into());
        }
    }
}

/// Scan Python dependencies and add required system packages for native extensions.
fn scan_python_native_deps(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    // Collect Python dependency names from all supported manifest files
    let dep_names = collect_python_dep_names(&project_dir);
    if dep_names.is_empty() {
        return;
    }

    let mut build_pkgs_to_add: Vec<String> = Vec::new();
    let mut runtime_pkgs_to_add: Vec<String> = Vec::new();

    for (pattern, build_deps, runtime_deps) in PYTHON_NATIVE_DEPS {
        if dep_names.iter().any(|d| dep_matches(d, pattern)) {
            for pkg in *build_deps {
                let pkg_str = pkg.to_string();
                if !build.build.packages.contains(&pkg_str) && !build_pkgs_to_add.contains(&pkg_str)
                {
                    build_pkgs_to_add.push(pkg_str);
                }
            }
            for pkg in *runtime_deps {
                let pkg_str = pkg.to_string();
                if !build.runtime.packages.contains(&pkg_str)
                    && !runtime_pkgs_to_add.contains(&pkg_str)
                {
                    runtime_pkgs_to_add.push(pkg_str);
                }
            }
        }
    }

    if !build_pkgs_to_add.is_empty() || !runtime_pkgs_to_add.is_empty() {
        // Native deps that compile C extensions need Python dev headers (Python.h).
        // Before Wolfi resolution, the package may be "python" (unversioned) or "python-3.X" (versioned).
        // Add the corresponding dev package so headers are available during build.
        if !build_pkgs_to_add.is_empty() {
            let dev_pkg = if let Some(py_pkg) = build
                .build
                .packages
                .iter()
                .find(|p| p.starts_with("python-3."))
            {
                Some(format!("{}-dev", py_pkg))
            } else if build.build.packages.iter().any(|p| p == "python") {
                // Unversioned — will be resolved by Wolfi; add unversioned dev placeholder
                Some("python-dev".to_string())
            } else {
                None
            };
            if let Some(dev) = dev_pkg {
                if !build.build.packages.contains(&dev) && !build_pkgs_to_add.contains(&dev) {
                    build_pkgs_to_add.push(dev);
                }
            }
        }

        debug!(
            build_pkgs = ?build_pkgs_to_add,
            runtime_pkgs = ?runtime_pkgs_to_add,
            "Adding system packages for Python native dependencies"
        );
        build.build.packages.extend(build_pkgs_to_add);
        build.runtime.packages.extend(runtime_pkgs_to_add);
    }
}

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
fn scan_node_native_deps(repo_root: &Path, build: &mut UniversalBuild) {
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
fn scan_node_system_deps(dep_names: &[String], build: &mut UniversalBuild) {
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
fn scan_node_puppeteer(repo_root: &Path, build: &mut UniversalBuild) {
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
fn sanitize_node_build_commands(repo_root: &Path, build: &mut UniversalBuild) {
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

/// Check if a dependency name matches a pattern (case-insensitive, handles extras like `psycopg[c]`).
fn dep_matches(dep_name: &str, pattern: &str) -> bool {
    let normalized = dep_name.to_lowercase().replace('-', "_");
    let pat_normalized = pattern.to_lowercase().replace('-', "_");
    normalized == pat_normalized || normalized.starts_with(&format!("{}[", pat_normalized))
}

/// Collect Python dependency names from requirements.txt, pyproject.toml, Pipfile, and uv.lock.
fn collect_python_dep_names(project_dir: &Path) -> Vec<String> {
    let mut deps = Vec::new();

    // requirements.txt
    if let Ok(content) = std::fs::read_to_string(project_dir.join("requirements.txt")) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }
            let name = trimmed
                .split(&['>', '<', '=', '~', '!', ';'][..])
                .next()
                .unwrap_or(trimmed)
                .trim();
            if !name.is_empty() {
                deps.push(name.to_string());
            }
        }
    }

    // pyproject.toml
    if let Ok(content) = std::fs::read_to_string(project_dir.join("pyproject.toml")) {
        if let Ok(toml_val) = toml::from_str::<toml::Value>(content.trim()) {
            // [project] dependencies
            if let Some(project_deps) = toml_val
                .get("project")
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_array())
            {
                for dep in project_deps {
                    if let Some(dep_str) = dep.as_str() {
                        let name = dep_str
                            .split(&['>', '<', '=', '~', '!', ';', ' '][..])
                            .next()
                            .unwrap_or(dep_str)
                            .trim();
                        if !name.is_empty() {
                            deps.push(name.to_string());
                        }
                    }
                }
            }

            // [tool.poetry.dependencies]
            if let Some(poetry_deps) = toml_val
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_table())
            {
                for name in poetry_deps.keys() {
                    if name != "python" {
                        deps.push(name.clone());
                    }
                }
            }
        }
    }

    // Pipfile
    if let Ok(content) = std::fs::read_to_string(project_dir.join("Pipfile")) {
        if let Ok(toml_val) = toml::from_str::<toml::Value>(content.trim()) {
            if let Some(packages) = toml_val.get("packages").and_then(|v| v.as_table()) {
                for name in packages.keys() {
                    deps.push(name.clone());
                }
            }
        }
    }

    deps
}

// ── Flask app path fix ────────────────────────────────────────────────────

/// Fix FLASK_APP for projects where the hardcoded `/build/app.py` doesn't exist.
/// Searches the project directory for `app.py` or `main.py` and updates accordingly.
/// If no Flask app file is found, falls back to a Python entrypoint command.
fn fix_flask_app_path(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    // Only fix if FLASK_APP is set to one of the default hardcoded values
    let flask_app = match build.runtime.env.get("FLASK_APP") {
        Some(v) if v == "/app/app.py" || v == "/build/app.py" => v.clone(),
        _ => return,
    };

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    let workdir = &build.runtime.workdir;

    // If app.py exists at project root, the default FLASK_APP is correct
    if project_dir.join("app.py").exists() && project_dir == repo_root {
        return;
    }

    // For workspace members in subdirectories, search for app.py in the project dir
    if project_dir != repo_root {
        // Check for app.py directly in the project directory
        if project_dir.join("app.py").exists() {
            let rel_path = project_dir
                .strip_prefix(repo_root)
                .unwrap_or(project_dir.as_path());
            let new_flask_app = format!("{}/{}/app.py", workdir, rel_path.display());
            debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP for workspace member");
            build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
            return;
        }

        // Search recursively for app.py in the project directory (e.g., src/api/app.py)
        for entry in WalkBuilder::new(&project_dir)
            .max_depth(Some(4))
            .build()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "app.py" && entry.file_type().is_some_and(|t| t.is_file()) {
                let rel_path = entry.path().strip_prefix(repo_root).unwrap_or(entry.path());
                let new_flask_app = format!("{}/{}", workdir, rel_path.display());
                debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP for workspace member");
                build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                return;
            }
        }
    }

    // No app.py found anywhere — check for main.py with Flask imports
    if project_dir.join("main.py").exists() {
        if let Ok(content) = std::fs::read_to_string(project_dir.join("main.py")) {
            if content.contains("from flask")
                || content.contains("import flask")
                || content.contains("import Flask")
                || content.contains("from Flask")
            {
                let workdir = &build.runtime.workdir;
                let new_flask_app = format!("{}/main.py", workdir);
                debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP to main.py (contains Flask imports)");
                build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                return;
            }
        }
    }

    // Search recursively for any Python file with Flask imports (e.g., package/__main__.py)
    for entry in WalkBuilder::new(&project_dir)
        .max_depth(Some(4))
        .build()
        .filter_map(|e| e.ok())
    {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.ends_with(".py"))
            && entry.file_type().is_some_and(|t| t.is_file())
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains("from flask") || content.contains("import flask") {
                    let rel_path = entry
                        .path()
                        .strip_prefix(&project_dir)
                        .unwrap_or(entry.path());
                    let workdir = &build.runtime.workdir;
                    // If it's a __main__.py inside a package directory, use the
                    // Python module import form for FLASK_APP. File paths don't
                    // work for __main__.py because Flask resolves it to the
                    // built-in __main__ module.
                    //
                    // This only works when the directory is a proper Python
                    // package (has __init__.py) so that import resolution
                    // succeeds. Without __init__.py, fall back to running the
                    // script directly via `python path/__main__.py`.
                    if rel_path.file_name().is_some_and(|n| n == "__main__.py") {
                        if let Some(parent) = rel_path.parent() {
                            let dir_name = parent.to_string_lossy();
                            if !dir_name.is_empty() {
                                let abs_parent = project_dir.join(parent);
                                if abs_parent.join("__init__.py").exists() {
                                    // Proper package — convert to importable module name:
                                    // - Replace hyphens with underscores (PEP 503)
                                    // - Replace path separators with dots
                                    let pkg_name = dir_name
                                        .replace('-', "_")
                                        .replace(std::path::MAIN_SEPARATOR, ".");
                                    let new_flask_app = format!("{}.__main__:app", pkg_name);
                                    build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                                    return;
                                }
                                // No __init__.py — not a proper Python package.
                                // Flask's `flask run` can't import __main__.py from
                                // non-package directories, so fall back to running
                                // the script directly.
                                debug!(
                                    path = %rel_path.display(),
                                    "Flask app in __main__.py without __init__.py, using direct Python execution"
                                );
                                let entrypoint = format!("{}/{}", workdir, rel_path.display());
                                build.runtime.env.remove("FLASK_APP");
                                build.runtime.env.remove("FLASK_RUN_HOST");
                                build.runtime.env.remove("FLASK_RUN_PORT");
                                build.runtime.command = vec!["python".into(), entrypoint];
                                return;
                            }
                        }
                    }
                    let new_flask_app = format!("{}/{}", workdir, rel_path.display());
                    build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                    return;
                }
            }
        }
    }

    // No Flask app file found at all — fall back to Python entrypoint
    // Remove Flask-specific runtime config and use a generic Python entrypoint
    debug!("No Flask app file found, falling back to Python entrypoint");
    build.runtime.env.remove("FLASK_APP");
    build.runtime.env.remove("FLASK_RUN_HOST");
    build.runtime.env.remove("FLASK_RUN_PORT");
    build.runtime.ports.clear();
    build.runtime.health = None;

    // Find a suitable Python entrypoint
    for filename in PYTHON_ENTRYPOINTS {
        if project_dir.join(filename).exists() {
            let workdir = &build.runtime.workdir;
            let entrypoint_path = format!("{}/{}", workdir, filename);
            debug!(entrypoint = %entrypoint_path, "Using Python entrypoint as Flask fallback");
            build.runtime.command = vec!["python".into(), entrypoint_path];
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{BuildSystemId, FrameworkId, LanguageId, RuntimeId};
    use std::collections::BTreeMap;

    const RUST: LanguageId = LanguageId::new("rust");
    const JAVA: LanguageId = LanguageId::new("java");
    const CARGO: BuildSystemId = BuildSystemId::new("cargo");
    const MAVEN: BuildSystemId = BuildSystemId::new("maven");
    const NATIVE: RuntimeId = RuntimeId::new("native");
    const JVM: RuntimeId = RuntimeId::new("jvm");
    const SPRING_BOOT: FrameworkId = FrameworkId::new("spring-boot");

    #[test]
    fn test_reduce_standalone_rust() {
        let bucket = ServiceBucket {
            path: PathBuf::new(),
            manifest: Manifest {
                path: PathBuf::from("Cargo.toml"),
                language: RUST,
                build_system: CARGO,
                runtime: NATIVE,
                package: Some(Package {
                    name: "my-app".into(),
                    version: Some("0.1.0".into()),
                    is_application: true,
                }),
                workspace: None,
                dependencies: vec![],
                build: BuildSpec {
                    packages: vec!["rust".into(), "build-base".into()],
                    commands: vec!["cargo build --release".into()],
                    member_transform: None,
                    env: BTreeMap::new(),
                    cache_dirs: vec!["target".into()],
                    artifacts: vec![("target/release/my-app".into(), "/app/my-app".into())],
                    build_image: None,
                },
                runtime_config: RuntimeSpec {
                    packages: vec!["ca-certificates".into()],
                    env: BTreeMap::new(),
                    entrypoint: Some("/app/my-app".into()),
                    workdir: Some("/app".into()),
                    ports: vec![8080],
                    health_endpoint: None,
                },
            },
            configs: vec![],
            framework: None,
            is_workspace_member: false,
            workspace_root: None,
        };

        let registry = Registry::with_defaults();
        let build = reduce(bucket, &registry).unwrap();
        assert_eq!(build.metadata.language, "Rust");
        assert_eq!(build.metadata.build_system, "Cargo");
        assert_eq!(build.build.commands, vec!["cargo build --release"]);
        assert_eq!(build.runtime.command, vec!["/app/my-app"]);
        assert_eq!(build.runtime.ports, vec![8080]);
    }

    #[test]
    fn test_reduce_workspace_member() {
        let bucket = ServiceBucket {
            path: PathBuf::from("api-service"),
            manifest: Manifest {
                path: PathBuf::from("api-service/pom.xml"),
                language: JAVA,
                build_system: MAVEN,
                runtime: JVM,
                package: Some(Package {
                    name: "api-service".into(),
                    version: Some("1.0.0".into()),
                    is_application: true,
                }),
                workspace: None,
                dependencies: vec![],
                build: BuildSpec {
                    packages: vec!["openjdk-21".into(), "maven".into()],
                    commands: vec!["mvn package -DskipTests".into()],
                    member_transform: Some(MemberBuildTransform {
                        member_commands: vec!["mvn -pl {module} -am install -DskipTests".into()],
                        member_artifacts: Some(vec![(
                            "{module}/target/*.jar".into(),
                            "/app/".into(),
                        )]),
                    }),
                    env: BTreeMap::new(),
                    cache_dirs: vec!["/root/.m2/repository/".into()],
                    artifacts: vec![("target/*.jar".into(), "/app/".into())],
                    build_image: None,
                },
                runtime_config: RuntimeSpec {
                    packages: vec!["openjdk-21".into()],
                    env: BTreeMap::new(),
                    entrypoint: Some("java -jar /app/api-service-1.0.0.jar".into()),
                    workdir: Some("/app".into()),
                    ports: vec![8080],
                    health_endpoint: None,
                },
            },
            configs: vec![],
            framework: Some(FrameworkContribution {
                framework: SPRING_BOOT,
                default_ports: vec![8080],
                health_endpoints: vec!["/actuator/health".into()],
                env_vars: BTreeMap::new(),
                runtime_packages: vec![],
                runtime_command: None,
                runtime_env: BTreeMap::new(),
                workdir: None,
                extra_copy: vec![],
            }),
            is_workspace_member: true,
            workspace_root: Some(PathBuf::new()),
        };

        let registry = Registry::with_defaults();
        let build = reduce(bucket, &registry).unwrap();
        assert_eq!(
            build.build.commands,
            vec!["mvn -pl api-service -am install -DskipTests"]
        );
        assert_eq!(build.runtime.copy[0].from, "api-service/target/*.jar");
        assert_eq!(build.metadata.framework, Some("Spring Boot".into()));
        assert_eq!(
            build.runtime.health,
            Some(HealthCheck {
                endpoint: "/actuator/health".into()
            })
        );
    }

    #[test]
    fn test_scan_source_health_express() {
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("index.js"),
            r#"
const express = require('express');
const app = express();

app.get('/healthz', (req, res) => {
    res.status(200).send('ok');
});

app.listen(3000);
"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("test".into()),
                language: "JavaScript".into(),
                build_system: "npm".into(),
                framework: Some("Express".into()),
                reasoning: "Detected from package.json".into(),
            },
            build: BuildStage {
                packages: vec![],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["npm".into(), "start".into()],
                workdir: "/app".into(),
                ports: vec![3000],
                health: None,
            },
        };

        scan_source_health(dir.path(), &mut build);
        assert_eq!(
            build.runtime.health,
            Some(HealthCheck {
                endpoint: "/healthz".into()
            })
        );
    }

    #[test]
    fn test_scan_source_health_does_not_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("index.js"),
            r#"app.get('/healthz', (req, res) => res.send('ok'));"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("test".into()),
                language: "JavaScript".into(),
                build_system: "npm".into(),
                framework: None,
                reasoning: "Detected from package.json".into(),
            },
            build: BuildStage {
                packages: vec![],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec![],
                workdir: "/app".into(),
                ports: vec![],
                health: Some(HealthCheck {
                    endpoint: "/actuator/health".into(),
                }),
            },
        };

        scan_source_health(dir.path(), &mut build);
        // Should NOT override the existing health endpoint
        assert_eq!(
            build.runtime.health,
            Some(HealthCheck {
                endpoint: "/actuator/health".into()
            })
        );
    }

    #[test]
    fn test_scan_source_env_vars_javascript() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.js"),
            r#"
const port = process.env.PORT || 3000;
const dbUrl = process.env.DATABASE_URL;
const home = process.env.HOME;
"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("test".into()),
                language: "JavaScript".into(),
                build_system: "npm".into(),
                framework: None,
                reasoning: "Detected from package.json".into(),
            },
            build: BuildStage {
                packages: vec![],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec![],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_source_env_vars(dir.path(), &mut build);
        assert!(build.runtime.env.contains_key("PORT"));
        assert!(build.runtime.env.contains_key("DATABASE_URL"));
        // HOME is a built-in var and should be skipped
        assert!(!build.runtime.env.contains_key("HOME"));
    }

    #[test]
    fn test_extract_project_dir() {
        let root = Path::new("/repo");
        assert_eq!(
            extract_project_dir(root, "Detected from package.json in api"),
            PathBuf::from("/repo/api")
        );
        assert_eq!(
            extract_project_dir(root, "Detected from Cargo.toml"),
            PathBuf::from("/repo")
        );
    }

    #[test]
    fn test_replace_package() {
        let mut packages = vec!["nodejs".into(), "npm".into(), "ca-certificates".into()];
        replace_package(&mut packages, "nodejs", "nodejs-20");
        assert_eq!(packages, vec!["nodejs-20", "npm", "ca-certificates"]);

        // Should not replace already-versioned packages
        replace_package(&mut packages, "nodejs", "nodejs-18");
        assert_eq!(packages[0], "nodejs-20");
    }

    #[test]
    fn test_cross_language_build_merge() {
        // Test that PHP + Node.js manifests in the same directory get merged,
        // with PHP as primary and Node.js build specs appended.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create .git directory so the walker treats it as a repo
        std::fs::create_dir_all(root.join(".git")).unwrap();

        // Create composer.json (PHP/Laravel)
        std::fs::write(
            root.join("composer.json"),
            r#"{
                "name": "laravel/laravel",
                "require": {
                    "php": "^8.2",
                    "laravel/framework": "^11.0"
                }
            }"#,
        )
        .unwrap();

        // Create package.json (Node.js/Vite for frontend assets)
        std::fs::write(
            root.join("package.json"),
            r#"{
                "private": true,
                "scripts": { "build": "vite build" },
                "dependencies": { "react": "^18.2.0" },
                "devDependencies": { "vite": "^5.0.0" }
            }"#,
        )
        .unwrap();

        let results = detect_without_wolfi(root).unwrap();

        // Should produce exactly one service (merged)
        assert_eq!(results.len(), 1);
        let build = &results[0];

        // Primary should be PHP/Composer (alphabetically first, server-side language)
        assert_eq!(build.metadata.language, "PHP");
        assert_eq!(build.metadata.build_system, "Composer");
        assert_eq!(build.metadata.framework.as_deref(), Some("Laravel"));

        // Build commands should include both Composer and npm commands
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("composer install")),
            "Should have composer command"
        );
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("npm install") || c.contains("npm ci")),
            "Should have npm install or npm ci command"
        );
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("npm run build")),
            "Should have npm run build command"
        );

        // Build packages should include both PHP and Node.js packages
        assert!(
            build.build.packages.iter().any(|p| p.starts_with("php")),
            "Should have PHP build packages"
        );
        assert!(
            build.build.packages.iter().any(|p| p.starts_with("nodejs")),
            "Should have Node.js build packages"
        );

        // Build cache should include both .composer/cache and .npm
        assert!(
            build.build.cache.contains(&".composer/cache".to_string()),
            "Should have composer cache"
        );
        assert!(
            build.build.cache.contains(&".npm".to_string()),
            "Should have npm cache"
        );

        // Runtime should be PHP-only (no Node.js packages)
        assert!(
            !build
                .runtime
                .packages
                .iter()
                .any(|p| p.starts_with("nodejs")),
            "Runtime should not have Node.js packages"
        );
        assert_eq!(build.runtime.ports, vec![8000]);
    }

    #[test]
    fn test_dep_matches_exact() {
        assert!(dep_matches("psycopg2", "psycopg2"));
        assert!(dep_matches("psycopg2-binary", "psycopg2-binary"));
        assert!(!dep_matches("psycopg2-binary", "psycopg2"));
        assert!(!dep_matches("psycopg2", "psycopg"));
    }

    #[test]
    fn test_dep_matches_extras() {
        assert!(dep_matches("psycopg[c]", "psycopg"));
        assert!(dep_matches("psycopg[binary]", "psycopg"));
        assert!(!dep_matches("psycopg2[binary]", "psycopg"));
    }

    #[test]
    fn test_dep_matches_case_insensitive() {
        assert!(dep_matches("Pillow", "pillow"));
        assert!(dep_matches("pillow", "Pillow"));
        assert!(dep_matches("Django", "django"));
    }

    #[test]
    fn test_dep_matches_underscore_hyphen() {
        assert!(dep_matches("psycopg2-binary", "psycopg2_binary"));
        assert!(dep_matches("psycopg2_binary", "psycopg2-binary"));
    }

    #[test]
    fn test_scan_python_native_deps_with_requirements() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("requirements.txt"),
            "psycopg2==2.9.3\nflask==3.0\n",
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "Detected from requirements.txt".into(),
            },
            build: BuildStage {
                packages: vec!["python-3.12".into(), "pip".into(), "build-base".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec!["python-3.12".into(), "libgcc".into()],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["python".into(), "app.py".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        assert!(
            build.build.packages.contains(&"postgresql-dev".to_string()),
            "Should add postgresql-dev for psycopg2"
        );
        assert!(
            build.runtime.packages.contains(&"libpq".to_string()),
            "Should add libpq for psycopg2 runtime"
        );
    }

    #[test]
    fn test_scan_python_native_deps_skips_non_python() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "psycopg2==2.9.3\n").unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Rust".into(),
                build_system: "cargo".into(),
                framework: None,
                reasoning: "Detected from Cargo.toml".into(),
            },
            build: BuildStage {
                packages: vec!["rust".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["./app".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        assert!(
            !build.build.packages.contains(&"postgresql-dev".to_string()),
            "Should not add postgresql-dev for non-Python project"
        );
    }

    #[test]
    fn test_scan_python_native_deps_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            r#"[project]
name = "myapp"
dependencies = [
    "mysqlclient>=2.1",
    "flask",
]
"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "Detected from pyproject.toml".into(),
            },
            build: BuildStage {
                packages: vec!["python-3.12".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec!["python-3.12".into()],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["python".into(), "app.py".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        assert!(
            build
                .build
                .packages
                .contains(&"mariadb-connector-c-dev".to_string()),
            "Should add mariadb-connector-c-dev for mysqlclient"
        );
        assert!(
            build
                .build
                .packages
                .contains(&"mariadb-connector-c".to_string()),
            "Should add mariadb-connector-c to build packages for mysqlclient (provides libmariadb.so.3)"
        );
        assert!(
            build
                .runtime
                .packages
                .contains(&"mariadb-connector-c".to_string()),
            "Should add mariadb-connector-c for mysqlclient runtime"
        );
    }

    #[test]
    fn test_scan_python_native_deps_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "psycopg2==2.9.3\n").unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "Detected from requirements.txt".into(),
            },
            build: BuildStage {
                packages: vec!["python-3.12".into(), "postgresql-dev".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec!["python-3.12".into(), "libpq".into()],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["python".into(), "app.py".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        let pg_count = build
            .build
            .packages
            .iter()
            .filter(|p| *p == "postgresql-dev")
            .count();
        assert_eq!(pg_count, 1, "Should not duplicate postgresql-dev");
    }
}

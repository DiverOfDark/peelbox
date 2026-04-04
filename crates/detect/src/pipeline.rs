//! Map-Reduce detection pipeline.
//!
//! Four steps: Parse → Detect Framework → Partition → Reduce.
//!
//! Build-system-specific behavior is delegated to `BuildSystemConfig` profiles
//! registered via `inventory::submit!` in the parser files.

use crate::helpers::{extract_project_dir, replace_package};
use crate::parsers::config::mise::scan_mise_config;
use crate::parsers::manifest::cargo_toml::{read_rust_version, resolve_rust_toolchain};
use crate::parsers::manifest::composer_json::read_php_version;
use crate::parsers::manifest::gemfile::read_ruby_version;
use crate::parsers::manifest::package_json::{
    detect_react_router_spa, ensure_npm_node_gyp, provide_framework_fallback_entrypoint,
    read_node_version, resolve_node_version, sanitize_node_build_commands, scan_node_native_deps,
    scan_node_puppeteer, wrap_yarn_corepack_entrypoint,
};
use crate::parsers::manifest::package_swift::read_swift_version;
use crate::parsers::manifest::pom_xml::{
    maven_build_image_for_version, read_java_version, resolve_java_toolchain,
    sync_java_home_with_packages,
};
use crate::parsers::manifest::pyproject_toml::{
    fix_django_settings, fix_flask_app_path, read_python_version, scan_python_entrypoints,
    scan_python_native_deps,
};
use crate::registry::Registry;
use crate::source_scanning::{scan_source_env_vars, scan_source_health, scan_source_ports};
use crate::traits::{ConfigParser, ManifestParser};
use crate::types::*;

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

    // Step 4j: Ensure npm builds have node-gyp available (Wolfi npm doesn't bundle it)
    for build in &mut builds {
        ensure_npm_node_gyp(build);
    }

    // Step 4k: Detect Node.js native dependency system packages
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
    // Skip build-side resolution when a custom build image is set (detector owns the setup).
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            if build.build.build_image.is_none() {
                resolve_wolfi_packages(&mut build.build.packages, wolfi);
            }
            resolve_wolfi_packages(&mut build.runtime.packages, wolfi);
        }
    }

    // Step 5b: Sync JAVA_HOME with resolved openjdk package version.
    // When a Docker image is used, set the Eclipse Temurin JAVA_HOME instead of Wolfi's.
    // BuildKit LLB doesn't inherit env from the base image, so we must set it explicitly.
    for build in &mut builds {
        if build.build.build_image.is_none() {
            sync_java_home_with_packages(build);
        } else {
            let lang = &build.metadata.language;
            if lang == "Java" || lang == "Kotlin" || lang == "Scala" || lang == "Clojure" {
                let java_home = "/opt/java/openjdk";
                let path = format!(
                    "{}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    java_home
                );
                build
                    .build
                    .env
                    .insert("JAVA_HOME".to_string(), java_home.to_string());
                build.build.env.insert("PATH".to_string(), path);
            }
        }
    }

    // Step 6: Handle pinned versions not available in Wolfi (use alternative installers)
    // Skip when a custom build image is set (Docker Hub has all versions).
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            if build.build.build_image.is_none() {
                resolve_rust_toolchain(build, wolfi);
                resolve_node_version(build, wolfi);
            }
        }
    }

    // Step 7: Handle old Java versions not available in Wolfi (use Adoptium Temurin)
    // Skip when a custom build image is set.
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            if build.build.build_image.is_none() {
                resolve_java_toolchain(build, wolfi);
            }
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

    // Step 9b: Detect React Router SPA mode and switch to vite preview
    for build in &mut builds {
        detect_react_router_spa(repo_path, build);
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
                            // runtime workdir. This ensures UV projects (workdir /app)
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

            // Cross-language merge: when Node.js is a secondary language (e.g.,
            // Laravel+Vite, Rails+esbuild), extract it into an asset_build phase
            // that runs in the Node Docker image before the primary build.
            for other in manifests_in_dir.iter() {
                if other.manifest.language == primary.manifest.language {
                    continue;
                }
                let is_node = matches!(other.manifest.language.slug(), "javascript" | "typescript");
                if is_node && primary.manifest.build.asset_build.is_none() {
                    primary.manifest.build.asset_build = Some(AssetBuild {
                        build_image: other
                            .manifest
                            .build
                            .build_image
                            .clone()
                            .unwrap_or_else(|| "docker.io/library/node:lts".into()),
                        commands: other.manifest.build.commands.clone(),
                        cache_dirs: other.manifest.build.cache_dirs.clone(),
                        env: other.manifest.build.env.clone(),
                    });
                }
                // Non-Node secondary languages are silently dropped.
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
    // For workspace members with adjusts_workspace_member_workdir, or standalone
    // subdirectory projects, set workdir to the member/subdir's directory so that
    // the entrypoint command runs in the correct context.
    let needs_subdir_workdir = (bucket.is_workspace_member || is_subdirectory)
        && profile.is_some_and(|p| p.adjusts_workspace_member_workdir);
    let workdir = if needs_subdir_workdir {
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
            asset_build: m.build.asset_build.as_ref().map(|ab| {
                peelbox_core::output::schema::AssetBuild {
                    build_image: ab.build_image.clone(),
                    commands: ab.commands.clone(),
                    cache: ab.cache_dirs.clone(),
                    env: ab.env.clone(),
                }
            }),
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
                if build.build.build_image.is_some() {
                    // Update Docker image tag with the pinned version
                    build.build.build_image = Some(format!("docker.io/library/node:{}", version));
                } else {
                    // Wolfi fallback (e.g., Bun): replace generic package
                    let versioned_pkg = format!("nodejs-{}", version);
                    replace_package(&mut build.build.packages, "nodejs", &versioned_pkg);
                }
                // Runtime always uses Wolfi packages
                let versioned_pkg = format!("nodejs-{}", version);
                replace_package(&mut build.runtime.packages, "nodejs", &versioned_pkg);
            }
        }
        "Python" => {
            if let Some(version) = read_python_version(&project_dir, repo_root) {
                if build.build.build_image.is_some() {
                    build.build.build_image = Some(format!("docker.io/library/python:{}", version));
                } else {
                    let versioned_pkg = format!("python-{}", version);
                    replace_package(&mut build.build.packages, "python", &versioned_pkg);
                }
                let versioned_pkg = format!("python-{}", version);
                replace_package(&mut build.runtime.packages, "python", &versioned_pkg);
            }
        }
        "PHP" => {
            if let Some(version) = read_php_version(&project_dir, repo_root) {
                if build.build.build_image.is_some() {
                    build.build.build_image =
                        Some(format!("docker.io/library/php:{}-cli", version));
                } else {
                    let versioned_pkg = format!("php-{}", version);
                    replace_package(&mut build.build.packages, "php", &versioned_pkg);
                }
                // Runtime always uses Wolfi packages
                let versioned_pkg = format!("php-{}", version);
                replace_package(&mut build.runtime.packages, "php", &versioned_pkg);
            }
        }
        "Ruby" => {
            if let Some(version) = read_ruby_version(&project_dir, repo_root) {
                if build.build.build_image.is_some() {
                    build.build.build_image = Some(format!("docker.io/library/ruby:{}", version));
                } else {
                    let versioned_pkg = format!("ruby-{}", version);
                    replace_package(&mut build.build.packages, "ruby", &versioned_pkg);
                }
                // Runtime always uses Wolfi packages
                let versioned_pkg = format!("ruby-{}", version);
                let versioned_dev = format!("ruby-{}-dev", version);
                replace_package(&mut build.build.packages, "ruby-dev", &versioned_dev);
                replace_package(&mut build.runtime.packages, "ruby", &versioned_pkg);
            }
        }
        "Rust" => {
            if let Some(version) = read_rust_version(&project_dir, repo_root) {
                if build.build.build_image.is_some() {
                    // Update Docker image tag with the pinned version
                    build.build.build_image = Some(format!("docker.io/library/rust:{}", version));
                } else {
                    // Wolfi fallback: replace generic package with versioned one
                    let versioned_pkg = format!("rust-{}", version);
                    replace_package(&mut build.build.packages, "rust", &versioned_pkg);
                }
            }
        }
        "Swift" => {
            // Override the Swift Docker image from .swift-version.
            // Each Swift major.minor needs a specific Ubuntu codename:
            // 5.4-5.6 → focal, 5.7-5.10 → jammy, 6.0+ → noble
            if let Some(version) = read_swift_version(&project_dir, repo_root) {
                let ubuntu_codename = match version.as_str() {
                    v if v.starts_with("5.4") || v.starts_with("5.5") || v.starts_with("5.6") => {
                        "focal"
                    }
                    v if v.starts_with("5.") => "jammy",
                    _ => "noble",
                };
                build.build.build_image = Some(format!(
                    "docker.io/library/swift:{}-{}",
                    version, ubuntu_codename
                ));
            }
        }
        "Java" | "Kotlin" | "Scala" | "Clojure" => {
            if let Some(version) = read_java_version(&project_dir, repo_root) {
                if let Some(ref image) = build.build.build_image {
                    // Update Docker image tag with the pinned JDK version.
                    // Maven images: maven:3-eclipse-temurin-{jdk}
                    // Gradle images: gradle:{ver}-jdk{jdk} or gradle:latest-jdk{jdk}
                    if image.contains("/maven:") {
                        build.build.build_image = Some(maven_build_image_for_version(&version));
                    } else if image.contains("/gradle:") {
                        // Preserve the Gradle version prefix, just replace the JDK suffix
                        if let Some(jdk_pos) = image.find("-jdk") {
                            let prefix = &image[..jdk_pos];
                            build.build.build_image = Some(format!("{}-jdk{}", prefix, version));
                        }
                    }
                } else {
                    // Wolfi fallback: replace generic openjdk package with versioned one
                    let versioned_pkg = format!("openjdk-{}", version);
                    replace_package(&mut build.build.packages, "openjdk", &versioned_pkg);
                }
                // Runtime always uses Wolfi packages
                let versioned_jre = format!("openjdk-{}-jre", version);
                replace_package(&mut build.runtime.packages, "openjdk", &versioned_jre);
            }
        }
        _ => {}
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
                    asset_build: None,
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
                    asset_build: None,
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
                asset_build: None,
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
                asset_build: None,
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
                asset_build: None,
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

        // Should produce exactly one service (PHP primary with Node.js asset_build)
        assert_eq!(results.len(), 1);
        let build = &results[0];

        // Primary should be PHP/Composer
        assert_eq!(build.metadata.language, "PHP");
        assert_eq!(build.metadata.build_system, "Composer");
        assert_eq!(build.metadata.framework.as_deref(), Some("Laravel"));

        // Primary build commands should be PHP-only (not merged with Node)
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("composer install")),
            "Should have composer command"
        );
        assert!(
            !build.build.commands.iter().any(|c| c.contains("npm")),
            "Primary build should NOT have npm commands (they're in asset_build)"
        );

        // Node.js should be in asset_build, not in primary build
        let asset = build
            .build
            .asset_build
            .as_ref()
            .expect("Should have asset_build");
        assert!(
            asset.build_image.contains("node"),
            "Asset build image should be Node: {}",
            asset.build_image
        );
        assert!(
            asset.commands.iter().any(|c| c.contains("npm")),
            "Asset build should have npm commands"
        );

        // Runtime should be PHP-only
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
}

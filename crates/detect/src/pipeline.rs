//! Map-Reduce detection pipeline.
//!
//! Four steps: Parse → Detect Framework → Partition → Reduce.

use crate::ids::{BuildSystemId, FrameworkId};
use crate::registry::Registry;
use crate::traits::{ConfigParser, FrameworkDetector, ManifestParser};
use crate::types::*;

// Build system IDs used in pipeline logic
const MAVEN: BuildSystemId = BuildSystemId::new("maven");
const CARGO: BuildSystemId = BuildSystemId::new("cargo");
const NPM: BuildSystemId = BuildSystemId::new("npm");
const PNPM: BuildSystemId = BuildSystemId::new("pnpm");
const YARN: BuildSystemId = BuildSystemId::new("yarn");
const BUN: BuildSystemId = BuildSystemId::new("bun");
const GRADLE: BuildSystemId = BuildSystemId::new("gradle");
const POETRY: BuildSystemId = BuildSystemId::new("poetry");
const PIP: BuildSystemId = BuildSystemId::new("pip");

// Framework IDs used in pipeline logic
const FLASK: FrameworkId = FrameworkId::new("flask");
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
    let manifests_with_frameworks = detect_frameworks(&repo_tree, &registry.framework_detectors);

    // Step 3: Partition into service buckets
    let buckets = partition(&repo_tree, manifests_with_frameworks);

    info!(services = buckets.len(), "Partitioned into service buckets");

    // Step 4: Reduce each bucket into UniversalBuild
    let mut builds: Vec<UniversalBuild> = buckets
        .into_iter()
        .filter_map(|bucket| match reduce(bucket) {
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

    // Step 4f: Scan Python entrypoints
    for build in &mut builds {
        scan_python_entrypoints(repo_path, build);
    }

    // Step 5: Resolve Wolfi package versions
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            resolve_wolfi_packages(&mut build.build.packages, wolfi);
            resolve_wolfi_packages(&mut build.runtime.packages, wolfi);
        }
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

fn detect_frameworks(
    tree: &RepoTree,
    detectors: &[Box<dyn FrameworkDetector>],
) -> Vec<ManifestWithFramework> {
    let mut results = Vec::new();
    collect_manifests_with_frameworks(&tree.tree, detectors, &mut results);
    results
}

fn collect_manifests_with_frameworks(
    node: &DirNode,
    detectors: &[Box<dyn FrameworkDetector>],
    results: &mut Vec<ManifestWithFramework>,
) {
    for file in &node.files {
        if let FileKind::Manifest(manifest) = &file.kind {
            let mut framework = detectors.iter().find_map(|detector| {
                if !detector.compatible_languages().contains(&manifest.language) {
                    return None;
                }
                if detector.detect(&manifest.dependencies) {
                    Some(detector.contribution(&manifest.dependencies))
                } else {
                    None
                }
            });

            // If Poetry build_system + Flask framework, swap in FlaskPoetry contribution
            if manifest.build_system == POETRY {
                if let Some(ref fw) = framework {
                    if fw.framework == FLASK {
                        // Find FlaskPoetryDetector and use its contribution instead
                        if let Some(poetry_fw) = detectors.iter().find_map(|d| {
                            let contrib = d.contribution(&manifest.dependencies);
                            if contrib.framework == FLASK
                                && contrib.runtime_env.contains_key("VIRTUAL_ENV")
                            {
                                Some(contrib)
                            } else {
                                None
                            }
                        }) {
                            framework = Some(poetry_fw);
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
        collect_manifests_with_frameworks(child, detectors, results);
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

fn partition(tree: &RepoTree, manifests: Vec<ManifestWithFramework>) -> Vec<ServiceBucket> {
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
                // Lock file parsers produce Pnpm or Yarn build_system
                if mwf.manifest.build_system == PNPM || mwf.manifest.build_system == YARN {
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

            // Merge framework from other manifests (unless primary is a lock file)
            if primary.framework.is_none() && lock_file_idx.is_none() {
                for other in manifests_in_dir.iter() {
                    if other.framework.is_some() {
                        primary.framework = other.framework.clone();
                        break;
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

            // Remove workspace root from merged so it doesn't become a standalone project.
            // Workspace roots coordinate member builds but are not deployable services themselves.
            merged.remove(ws_root);
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

fn reduce(bucket: ServiceBucket) -> Result<UniversalBuild> {
    let m = &bucket.manifest;

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
                        .replace("{root}", &bucket.workspace_root_display())
                })
                .collect()
        } else {
            m.build.commands.clone()
        }
    } else if is_subdirectory {
        // For standalone projects in subdirectories, prepend directory context
        m.build
            .commands
            .iter()
            .map(|cmd| {
                if m.build_system == MAVEN {
                    // Maven: use -f flag
                    if cmd.starts_with("mvn ") {
                        let mut result =
                            cmd.replacen("mvn ", &format!("mvn -f {}/pom.xml ", subdir), 1);
                        // For dependency:copy-dependencies, ensure the target dir exists
                        if cmd.contains("dependency:copy-dependencies") {
                            result = format!("{}; mkdir -p {}/target/lib", result, subdir);
                        }
                        result
                    } else {
                        format!("cd {} && {}", subdir, cmd)
                    }
                } else if m.build_system == CARGO {
                    // Cargo: use --manifest-path flag
                    if cmd.starts_with("cargo ") {
                        format!(
                            "{} --manifest-path {}/Cargo.toml --target-dir target",
                            cmd, subdir
                        )
                    } else {
                        format!("cd {} && {}", subdir, cmd)
                    }
                } else {
                    // npm, pnpm, yarn, bun, and others: use cd prefix
                    format!("cd {} && {}", subdir, cmd)
                }
            })
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
                        from: from.replace("{module}", &bucket.module_name()),
                        to: to.replace("{module}", &bucket.module_name()),
                    })
                    .collect()
            } else {
                m.build
                    .artifacts
                    .iter()
                    .map(|(from, to)| CopySpec {
                        from: from.replace("{module}", &bucket.module_name()),
                        to: to.replace("{module}", &bucket.module_name()),
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
        // Exception: Cargo projects use --target-dir target, so artifacts are at repo root
        let uses_shared_target = m.build_system == CARGO;
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

    // Gradle-specific: replace glob artifacts with specific jar name when package has name+version
    if m.build_system == GRADLE {
        if let Some(pkg) = &m.package {
            if let Some(version) = &pkg.version {
                let specific_jar = format!("{}-{}.jar", pkg.name, version);
                for artifact in &mut artifacts {
                    if artifact.from.contains("*.jar") {
                        artifact.from = artifact.from.replace("*.jar", &specific_jar);
                    }
                }
            }
        }
    }

    // Merge config contributions into runtime spec
    let mut runtime_env = m.runtime_config.env.clone();
    let mut runtime_ports = m.runtime_config.ports.clone();
    let mut runtime_packages = m.runtime_config.packages.clone();
    let mut health_endpoint = m.runtime_config.health_endpoint.clone();

    for config in &bucket.configs {
        runtime_env.extend(config.env_vars.clone());
        runtime_ports.extend(config.ports.clone());
        if health_endpoint.is_none() {
            health_endpoint.clone_from(&config.health_endpoint);
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

    // Deduplicate (preserve order, remove later duplicates)
    runtime_ports.sort();
    runtime_ports.dedup();
    {
        let mut seen = std::collections::HashSet::new();
        runtime_packages.retain(|p| seen.insert(p.clone()));
    }

    // Determine project name
    let is_root_project = bucket.path.as_os_str().is_empty() || bucket.path == Path::new(".");
    let project_name = if is_root_project {
        // For root-level projects: use package name only from strong naming sources
        // (npm, cargo, zig), not from settings.gradle or pyproject.toml
        if m.build_system == GRADLE || m.build_system == POETRY || m.build_system == PIP {
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

    // Build entrypoint command: framework override > manifest entrypoint
    let entrypoint_cmd = if let Some(fw_cmd) = framework_runtime_command {
        fw_cmd
    } else if let Some(entrypoint) = &m.runtime_config.entrypoint {
        // For non-root Node.js projects, use pkg_manager start instead of direct command
        if !is_root_project
            && (m.build_system == NPM
                || m.build_system == PNPM
                || m.build_system == YARN
                || m.build_system == BUN)
        {
            let pkg_manager = if m.build_system == YARN {
                "yarn"
            } else if m.build_system == PNPM {
                "pnpm"
            } else if m.build_system == BUN {
                "bun"
            } else {
                "npm"
            };
            vec![pkg_manager.into(), "start".into()]
        } else {
            entrypoint.split_whitespace().map(String::from).collect()
        }
    } else {
        vec![]
    };

    // Workdir: framework override > manifest workdir
    // For Node.js workspace members, set workdir to the member's directory
    // so that `npm start` finds the member's package.json
    let workdir = if bucket.is_workspace_member
        && (m.build_system == NPM
            || m.build_system == PNPM
            || m.build_system == YARN
            || m.build_system == BUN)
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
            if let Some(resolved) = wolfi.get_latest_version(prefix) {
                debug!(from = %pkg, to = %resolved, "Resolved Wolfi package version");
                *pkg = resolved;
            }
        } else if pkg.ends_with("-dev") {
            // Handle e.g. erlang-dev → erlang-28-dev
            let base = pkg.strip_suffix("-dev").unwrap();
            if let Some((_, prefix)) = VERSIONABLE_PACKAGES.iter().find(|(name, _)| *name == base) {
                if let Some(resolved) = wolfi.get_latest_version(prefix) {
                    let dev_pkg = format!("{}-dev", resolved);
                    if wolfi.has_package(&dev_pkg) {
                        debug!(from = %pkg, to = %dev_pkg, "Resolved Wolfi dev package version");
                        *pkg = dev_pkg;
                    }
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

// ── Source code port scanning ────────────────────────────────────────────────

/// Language-specific regex patterns for detecting ports in source code.
/// Each entry is (language, extension_filter, patterns) where patterns are regex strings
/// with a capture group for the port number.
const PORT_PATTERNS: &[(&str, &[&str], &[&str])] = &[
    (
        "Rust",
        &["rs"],
        &[
            r#"[.:]+bind\([^,)]*:(\d{4,5})"#,
            r#"[.:]+bind\(\("[^"]*",\s*(\d{4,5})\)"#,
            r#"addr\s*=\s*"[^:]*:(\d{4,5})""#,
        ],
    ),
    (
        "JavaScript",
        &["js", "ts", "mjs", "cjs"],
        &[r"\.listen\(\s*(\d{4,5})", r#"port["\s:=]+(\d{4,5})"#],
    ),
    (
        "TypeScript",
        &["js", "ts", "mjs", "cjs"],
        &[r"\.listen\(\s*(\d{4,5})", r#"port["\s:=]+(\d{4,5})"#],
    ),
    (
        "Python",
        &["py"],
        &[
            r"\.run\([^)]*port\s*=\s*(\d{4,5})",
            r#"port\s*=\s*(\d{4,5})"#,
        ],
    ),
    (
        "Go",
        &["go"],
        &[
            r"ListenAndServe\([^)]*:(\d{4,5})",
            r#"addr\s*=\s*"[^:]*:(\d{4,5})""#,
        ],
    ),
    (
        "Java",
        &["java", "kt", "kts"],
        &[
            r"\.setPort\(\s*(\d{4,5})\s*\)",
            r#"server\.port\s*=\s*(\d{4,5})"#,
        ],
    ),
    (
        "Kotlin",
        &["java", "kt", "kts"],
        &[
            r"\.setPort\(\s*(\d{4,5})\s*\)",
            r#"server\.port\s*=\s*(\d{4,5})"#,
        ],
    ),
    ("Elixir", &["ex", "exs"], &[r#"port:\s*(\d{4,5})"#]),
    (
        "Ruby",
        &["rb"],
        &[r#"set\s*:port\s*,\s*(\d{4,5})"#, r#"port\s*=\s*(\d{4,5})"#],
    ),
    (
        "C#",
        &["cs"],
        &[
            r#"UseUrls\([^)]*:(\d{4,5})"#,
            r#"app\.Run\([^)]*:(\d{4,5})"#,
            r#"\.UsePort\(\s*(\d{4,5})\s*\)"#,
        ],
    ),
    (
        "F#",
        &["fs"],
        &[
            r#"UseUrls\([^)]*:(\d{4,5})"#,
            r#"\.UsePort\(\s*(\d{4,5})\s*\)"#,
        ],
    ),
    (
        "PHP",
        &["php"],
        &[r#"'PORT'\s*,\s*(\d{4,5})"#, r#"\$port\s*=\s*(\d{4,5})"#],
    ),
];

/// Scan source files in a project directory for port patterns.
/// When ports are found in source code, they replace the framework default ports.
fn scan_source_ports(repo_root: &Path, build: &mut UniversalBuild) {
    let language = &build.metadata.language;

    let patterns_entry = PORT_PATTERNS.iter().find(|(lang, _, _)| *lang == language);

    let (_, extensions, patterns) = match patterns_entry {
        Some(entry) => entry,
        None => return,
    };

    let compiled: Vec<regex::Regex> = patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    if compiled.is_empty() {
        return;
    }

    // Determine the project directory to scan
    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    if !project_dir.is_dir() {
        return;
    }

    let mut source_ports: Vec<u16> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Walk source files with matching extensions
    let walker = WalkBuilder::new(&project_dir)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if !extensions.contains(&ext) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for re in &compiled {
            for cap in re.captures_iter(&content) {
                if let Some(port_match) = cap.get(1) {
                    if let Ok(port) = port_match.as_str().parse::<u16>() {
                        if port >= 1024 && seen.insert(port) {
                            debug!(port, file = %path.display(), "Found port in source code");
                            source_ports.push(port);
                        }
                    }
                }
            }
        }
    }

    // Source-code ports override framework defaults when found
    if !source_ports.is_empty() {
        source_ports.sort();
        source_ports.dedup();
        build.runtime.ports = source_ports;
    }
}

// ── Source code health endpoint scanning ──────────────────────────────────

/// Language-specific regex patterns for detecting health endpoints in source code.
const HEALTH_PATTERNS: &[(&str, &[&str], &[&str])] = &[
    (
        "JavaScript",
        &["js", "ts", "mjs", "cjs"],
        &[r#"app\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        "TypeScript",
        &["js", "ts", "mjs", "cjs"],
        &[r#"app\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        "Java",
        &["java", "kt", "kts"],
        &[r#"@(?:Get|Request)Mapping\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        "Kotlin",
        &["java", "kt", "kts"],
        &[r#"@(?:Get|Request)Mapping\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        "Python",
        &["py"],
        &[r#"@app\.(?:get|route)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        "Go",
        &["go"],
        &[r#"\.(?:GET|Handle(?:Func)?)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        "Rust",
        &["rs"],
        &[r#"\.(?:get|route)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
];

/// Scan source files for health endpoint patterns.
/// Only sets health if not already set by config or framework.
fn scan_source_health(repo_root: &Path, build: &mut UniversalBuild) {
    if build.runtime.health.is_some() {
        return;
    }

    let language = &build.metadata.language;
    let patterns_entry = HEALTH_PATTERNS
        .iter()
        .find(|(lang, _, _)| *lang == language);
    let (_, extensions, patterns) = match patterns_entry {
        Some(entry) => entry,
        None => return,
    };

    let compiled: Vec<regex::Regex> = patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    if compiled.is_empty() {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    if !project_dir.is_dir() {
        return;
    }

    let walker = WalkBuilder::new(&project_dir)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !extensions.contains(&ext) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for re in &compiled {
            if let Some(cap) = re.captures(&content) {
                if let Some(endpoint) = cap.get(1) {
                    let ep = endpoint.as_str().to_string();
                    debug!(endpoint = %ep, file = %path.display(), "Found health endpoint in source code");
                    build.runtime.health = Some(HealthCheck { endpoint: ep });
                    return;
                }
            }
        }
    }
}

// ── Source code env var scanning ──────────────────────────────────────────

/// Language-specific regex patterns for detecting environment variable access.
const ENV_VAR_PATTERNS: &[(&str, &[&str], &[&str])] = &[
    (
        "JavaScript",
        &["js", "ts", "mjs", "cjs"],
        &[r"process\.env\.([A-Z_][A-Z0-9_]*)"],
    ),
    (
        "TypeScript",
        &["js", "ts", "mjs", "cjs"],
        &[r"process\.env\.([A-Z_][A-Z0-9_]*)"],
    ),
    (
        "Python",
        &["py"],
        &[
            r#"os\.environ\.get\(['"]([A-Z_][A-Z0-9_]*)['"]"#,
            r#"os\.getenv\(['"]([A-Z_][A-Z0-9_]*)['"]"#,
        ],
    ),
    ("Rust", &["rs"], &[r#"env::var\(["']([A-Z_][A-Z0-9_]*)"#]),
    ("Go", &["go"], &[r#"os\.Getenv\(["']([A-Z_][A-Z0-9_]*)"#]),
    (
        "Java",
        &["java", "kt", "kts"],
        &[r#"System\.getenv\(["']([A-Z_][A-Z0-9_]*)"#],
    ),
    (
        "Kotlin",
        &["java", "kt", "kts"],
        &[r#"System\.getenv\(["']([A-Z_][A-Z0-9_]*)"#],
    ),
];

/// Built-in environment variables to skip.
const BUILTIN_ENV_VARS: &[&str] = &["PATH", "HOME", "USER", "SHELL", "LANG", "TERM"];

/// Scan source files for environment variable references.
/// Adds discovered vars to runtime env with empty values (only if not already present).
fn scan_source_env_vars(repo_root: &Path, build: &mut UniversalBuild) {
    let language = &build.metadata.language;
    let patterns_entry = ENV_VAR_PATTERNS
        .iter()
        .find(|(lang, _, _)| *lang == language);
    let (_, extensions, patterns) = match patterns_entry {
        Some(entry) => entry,
        None => return,
    };

    let compiled: Vec<regex::Regex> = patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    if compiled.is_empty() {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    if !project_dir.is_dir() {
        return;
    }

    let walker = WalkBuilder::new(&project_dir)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !extensions.contains(&ext) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for re in &compiled {
            for cap in re.captures_iter(&content) {
                if let Some(var_match) = cap.get(1) {
                    let var_name = var_match.as_str();
                    if BUILTIN_ENV_VARS.contains(&var_name) {
                        continue;
                    }
                    if !build.runtime.env.contains_key(var_name) {
                        debug!(var = var_name, file = %path.display(), "Found env var in source code");
                        build
                            .runtime
                            .env
                            .insert(var_name.to_string(), String::new());
                    }
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
        _ => {}
    }
}

/// Read Node.js version from .nvmrc or .node-version file.
fn read_node_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
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
fn parse_node_version_string(s: &str) -> Option<String> {
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

/// Read Python version from .python-version file or Pipfile.
fn read_python_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    // Check .python-version first (highest priority)
    for dir in &[project_dir, repo_root] {
        let path = dir.join(".python-version");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            // Extract major.minor (e.g., "3.11.4" → "3.11")
            let parts: Vec<&str> = trimmed.split('.').collect();
            if parts.len() >= 2
                && parts[0].chars().all(|c| c.is_ascii_digit())
                && parts[1].chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("{}.{}", parts[0], parts[1]));
            }
        }
    }

    // Check Pipfile for python_version in [requires] section
    for dir in &[project_dir, repo_root] {
        let path = dir.join("Pipfile");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(version) = parse_pipfile_python_version(&content) {
                return Some(version);
            }
        }
    }

    None
}

/// Extract python_version from Pipfile [requires] section.
/// Matches `python_version = "3.11"` or `python_version = "3"`.
fn parse_pipfile_python_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*python_version\s*=\s*["'](\d+\.\d+)["']"#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Replace an unversioned package with a versioned one.
/// Only replaces exact matches (not already-versioned packages).
fn replace_package(packages: &mut [String], unversioned: &str, versioned: &str) {
    for pkg in packages.iter_mut() {
        if pkg == unversioned {
            *pkg = versioned.to_string();
        }
    }
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

// ── Helper ───────────────────────────────────────────────────────────────

/// Extract the project directory from the build metadata reasoning string.
fn extract_project_dir(repo_root: &Path, reasoning: &str) -> PathBuf {
    if let Some(in_pos) = reasoning.rfind(" in ") {
        let dir = &reasoning[in_pos + 4..];
        repo_root.join(dir)
    } else {
        repo_root.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{FrameworkId, LanguageId, RuntimeId};
    use std::collections::BTreeMap;

    const RUST: LanguageId = LanguageId::new("rust");
    const JAVA: LanguageId = LanguageId::new("java");
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

        let build = reduce(bucket).unwrap();
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

        let build = reduce(bucket).unwrap();
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
}

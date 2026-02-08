//! Map-Reduce detection pipeline.
//!
//! Four steps: Parse → Detect Framework → Partition → Reduce.

use crate::registry::Registry;
use crate::traits::{ConfigParser, FrameworkDetector, ManifestParser};
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

    // Step 5: Resolve Wolfi package versions
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            resolve_wolfi_packages(&mut build.build.packages, wolfi);
            resolve_wolfi_packages(&mut build.runtime.packages, wolfi);
        }
    }

    // Filter out non-application builds (e.g., library-only workspaces)
    builds.retain(|b| {
        !b.build.commands.is_empty()
            || !b.runtime.command.is_empty()
            || b.metadata.project_name.is_some()
    });

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

        let filename = rel_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let dir = rel_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        let kind = classify_file(
            &abs_path,
            &rel_path,
            filename,
            &manifest_lookup,
            &config_lookup,
        );

        file_map
            .entry(dir)
            .or_default()
            .push(TypedFile { path: rel_path, kind });
    }

    // Build hierarchical tree from flat map
    let tree = build_dir_node(Path::new(""), &mut file_map);

    Ok(RepoTree {
        root: repo_path.to_path_buf(),
        tree,
    })
}

fn build_parser_lookup<'a>(
    parsers: &'a [Box<dyn ManifestParser>],
) -> HashMap<&'a str, &'a dyn ManifestParser> {
    let mut map = HashMap::new();
    for parser in parsers {
        for filename in parser.filenames() {
            map.insert(*filename, parser.as_ref());
        }
    }
    map
}

fn build_config_lookup<'a>(
    parsers: &'a [Box<dyn ConfigParser>],
) -> HashMap<&'a str, &'a dyn ConfigParser> {
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
            if let Some(manifest) = parser.parse(rel_path, &content) {
                debug!(file = %rel_path.display(), "Parsed manifest");
                return FileKind::Manifest(manifest);
            }
        }
    }

    // Try .csproj / .fsproj files (special case: extension-based matching)
    if filename.ends_with(".csproj") || filename.ends_with(".fsproj") {
        let csproj_parser = crate::parsers::manifest::CsprojParser;
        if let Ok(content) = std::fs::read_to_string(abs_path) {
            if let Some(manifest) = ManifestParser::parse(&csproj_parser, rel_path, &content) {
                debug!(file = %rel_path.display(), "Parsed .NET project file");
                return FileKind::Manifest(manifest);
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
            let framework = detectors.iter().find_map(|detector| {
                if !detector
                    .compatible_languages()
                    .contains(&manifest.language)
                {
                    return None;
                }
                if detector.detect(&manifest.dependencies) {
                    Some(detector.contribution())
                } else {
                    None
                }
            });

            results.push(ManifestWithFramework {
                path: file.path.clone(),
                manifest: manifest.clone(),
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

fn partition(
    tree: &RepoTree,
    manifests: Vec<ManifestWithFramework>,
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
            // Find the one with the most build info (non-empty commands)
            let mut primary_idx = 0;
            let mut workspace_idx = None;

            for (i, mwf) in manifests_in_dir.iter().enumerate() {
                if !mwf.manifest.build.commands.is_empty() {
                    primary_idx = i;
                }
                if mwf.manifest.workspace.is_some() {
                    workspace_idx = Some(i);
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

            // Merge package info
            if primary.manifest.package.is_none() {
                for other in manifests_in_dir.iter() {
                    if other.manifest.package.is_some() {
                        primary.manifest.package = other.manifest.package.clone();
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

    if !workspace_roots.is_empty() {
        // Workspace mode: expand member patterns
        for (ws_root, workspace) in &workspace_roots {
            let expanded_members = expand_workspace_members(
                &tree.root,
                ws_root,
                &workspace.members,
                &merged,
            );

            for member_dir in expanded_members {
                if let Some(mwf) = merged.remove(&member_dir) {
                    let configs = collect_configs_for_service(&member_dir, ws_root, &dir_configs);
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

            // If workspace root itself has a package and build commands, include it too
            if let Some(mwf) = merged.remove(ws_root) {
                if mwf.manifest.package.is_some() && !mwf.manifest.build.commands.is_empty() {
                    let configs =
                        collect_configs_for_service(ws_root, ws_root, &dir_configs);
                    buckets.push(ServiceBucket {
                        path: ws_root.clone(),
                        manifest: mwf.manifest,
                        configs,
                        framework: mwf.framework,
                        is_workspace_member: false,
                        workspace_root: None,
                    });
                }
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

    // Resolve build commands (workspace-aware)
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
    } else {
        m.build.commands.clone()
    };

    // Resolve artifacts (workspace-aware)
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
        if runtime_ports.is_empty() {
            runtime_ports = fw.default_ports.clone();
        }
        if health_endpoint.is_none() {
            health_endpoint = fw.health_endpoints.first().cloned();
        }
        framework_runtime_command = fw.runtime_command.clone();
        framework_workdir = fw.workdir.clone();

        // Add extra copy specs
        for (from, to) in &fw.extra_copy {
            artifacts.push(CopySpec {
                from: from.clone(),
                to: to.clone(),
            });
        }
        Some(fw.framework.name())
    } else {
        None
    };

    // Deduplicate (preserve order, remove later duplicates)
    runtime_ports.sort();
    runtime_ports.dedup();
    {
        let mut seen = std::collections::HashSet::new();
        runtime_packages.retain(|p| seen.insert(p.clone()));
    }

    // Determine project name
    let project_name = m.package.as_ref().map(|p| p.name.clone()).or_else(|| {
        // Fallback to directory name
        let dir_name = bucket.path.file_name()?.to_str()?.to_string();
        if dir_name.is_empty() {
            None
        } else {
            Some(dir_name)
        }
    });

    // Build entrypoint command: framework override > manifest entrypoint
    let entrypoint_cmd = if let Some(fw_cmd) = framework_runtime_command {
        fw_cmd
    } else {
        m.runtime_config
            .entrypoint
            .as_ref()
            .map(|e| e.split_whitespace().map(String::from).collect())
            .unwrap_or_default()
    };

    // Workdir: framework override > manifest workdir
    let workdir = framework_workdir
        .or_else(|| m.runtime_config.workdir.clone())
        .unwrap_or_else(|| "/app".into());

    Ok(UniversalBuild {
        version: "1.0".into(),
        metadata: BuildMetadata {
            project_name,
            language: m.language.name(),
            build_system: m.build_system.name(),
            framework: framework_name,
            reasoning: format!("Detected from {}", m.path.display()),
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
];

/// Resolve generic package names to versioned Wolfi package names.
fn resolve_wolfi_packages(packages: &mut Vec<String>, wolfi: &WolfiPackageIndex) {
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
        if let Some((_, prefix)) = VERSIONABLE_PACKAGES.iter().find(|(name, _)| *name == pkg.as_str()) {
            if let Some(resolved) = wolfi.get_latest_version(prefix) {
                debug!(from = %pkg, to = %resolved, "Resolved Wolfi package version");
                *pkg = resolved;
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
        } else if pkg.starts_with("dotnet-") && pkg.ends_with("-runtime") {
            if !wolfi.has_package(pkg) {
                if let Some(latest) = wolfi.get_latest_version("dotnet") {
                    let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                    *pkg = format!("dotnet-{}-runtime", ver);
                }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_reduce_standalone_rust() {
        let bucket = ServiceBucket {
            path: PathBuf::new(),
            manifest: Manifest {
                path: PathBuf::from("Cargo.toml"),
                language: peelbox_stack::LanguageId::Rust,
                build_system: peelbox_stack::BuildSystemId::Cargo,
                runtime: peelbox_stack::RuntimeId::Native,
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
                language: peelbox_stack::LanguageId::Java,
                build_system: peelbox_stack::BuildSystemId::Maven,
                runtime: peelbox_stack::RuntimeId::JVM,
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
                        member_commands: vec![
                            "mvn -pl {module} -am install -DskipTests".into(),
                        ],
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
                framework: peelbox_stack::FrameworkId::SpringBoot,
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
}

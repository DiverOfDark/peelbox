use super::*;

pub(crate) fn partition(
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
pub(crate) fn propagate_versioned_packages(
    member_packages: &mut [String],
    root_packages: &[String],
) {
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

pub(crate) fn expand_workspace_members(
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

pub(crate) fn collect_configs_for_service(
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

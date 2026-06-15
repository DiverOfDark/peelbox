use super::*;

pub(crate) struct ManifestWithFramework {
    pub(crate) path: PathBuf,
    pub(crate) manifest: Manifest,
    pub(crate) framework: Option<FrameworkContribution>,
}

pub(crate) fn detect_frameworks(
    tree: &RepoTree,
    registry: &Registry,
) -> Vec<ManifestWithFramework> {
    let mut results = Vec::new();
    collect_manifests_with_frameworks(&tree.tree, registry, &mut results);
    results
}

pub(crate) fn collect_manifests_with_frameworks(
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

pub(crate) fn collect_configs(node: &DirNode) -> Vec<ConfigContribution> {
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

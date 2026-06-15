use super::*;

pub(crate) fn build_tree(repo_path: &Path, registry: &Registry) -> Result<RepoTree> {
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

pub(crate) fn build_parser_lookup(
    parsers: &[Box<dyn ManifestParser>],
) -> HashMap<&str, &dyn ManifestParser> {
    let mut map = HashMap::new();
    for parser in parsers {
        for filename in parser.filenames() {
            map.insert(*filename, parser.as_ref());
        }
    }
    map
}

pub(crate) fn build_config_lookup(
    parsers: &[Box<dyn ConfigParser>],
) -> HashMap<&str, &dyn ConfigParser> {
    let mut map = HashMap::new();
    for parser in parsers {
        for filename in parser.filenames() {
            map.insert(*filename, parser.as_ref());
        }
    }
    map
}

pub(crate) fn classify_file(
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

pub(crate) fn build_dir_node(
    dir_path: &Path,
    file_map: &mut HashMap<PathBuf, Vec<TypedFile>>,
) -> DirNode {
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

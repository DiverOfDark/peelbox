use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

/// Detects Yarn projects by presence of yarn.lock.
/// Reads the sibling package.json for project info but overrides build system to Yarn.
pub struct YarnLockParser;

impl ManifestParser for YarnLockParser {
    fn filenames(&self) -> &[&str] {
        &["yarn.lock"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        // Read the sibling package.json
        let dir = path.parent()?;
        let pkg_json_path = dir.join("package.json");
        // Use the relative path for the sibling
        let abs_pkg_json = if pkg_json_path.is_absolute() {
            pkg_json_path
        } else {
            // During pipeline, the path is relative, we need to find it
            // The PackageJsonParser will be called separately; this parser
            // needs to read from absolute path resolved during tree walk.
            // We return None if we can't find it.
            return None;
        };
        let pkg_content = std::fs::read_to_string(&abs_pkg_json).ok()?;
        let json: serde_json::Value = serde_json::from_str(&pkg_content).ok()?;

        let name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_start = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .is_some();

        // Lock file parsers don't carry dependencies — framework detection
        // happens on the sibling package.json manifest instead.
        let dependencies: Vec<Dependency> = Vec::new();

        let language = LanguageId::JavaScript;

        let entrypoint = json
            .get("main")
            .and_then(|v| v.as_str())
            .map(|m| format!("node {}", m));

        // Detect tsc build script
        let has_tsc = json
            .get("scripts")
            .and_then(|s| s.get("build"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("tsc"))
            .unwrap_or(false);

        let build_cmd = if has_tsc {
            "./node_modules/.bin/tsc".to_string()
        } else {
            "yarn run build".to_string()
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Yarn,
            runtime: RuntimeId::Node,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "nodejs".into(),
                    "yarn".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "yarn install --network-timeout 100000 --network-concurrency 1".into(),
                    build_cmd,
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".yarn-cache".into(), "node_modules".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "nodejs".into(),
                    "npm".into(),
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

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(YarnLockParser))
}

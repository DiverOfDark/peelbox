//! Mix build system (Elixir)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use regex::Regex;
use std::path::{Path, PathBuf};

pub struct MixBuildSystem;

impl BuildSystem for MixBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Mix
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "mix.exs".to_string(),
            priority: 10,
        }]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for path in file_tree {
            if path.file_name().and_then(|n| n.to_str()) == Some("mix.exs") {
                detections.push(DetectionStack::new(
                    BuildSystemId::Mix,
                    LanguageId::Elixir,
                    path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let elixir_version = wolfi_index
            .get_latest_version("elixir")
            .expect("Failed to get elixir version from Wolfi index");

        let erlang_version = wolfi_index
            .get_latest_version("erlang")
            .unwrap_or_else(|| "erlang-28".to_string());

        // Separate dependency installation from compilation for better caching
        // deps.get fetches dependencies (cached), compile builds from source (not cached)
        let build_commands = vec![
            "mix deps.get".to_string(),
            "mix compile".to_string(),
        ];

        let runtime_copy = vec![(".".to_string(), "/app".to_string())];

        let runtime_env = std::collections::BTreeMap::from([
            ("PORT".to_string(), "4000".to_string()),
            ("MIX_ENV".to_string(), "prod".to_string()),
            ("LC_ALL".to_string(), "C.UTF-8".to_string()),
            ("ELIXIR_ERL_OPTIONS".to_string(), "+fnu".to_string()),
        ]);

        let build_env =
            std::collections::BTreeMap::from([("MIX_ENV".to_string(), "prod".to_string())]);

        BuildTemplate {
            build_packages: vec![
                elixir_version,
                erlang_version,
                "git".to_string(),
                "build-base".to_string(),
                "openssl".to_string(),
                "ca-certificates".to_string(),
            ],
            build_commands,
            cache_paths: vec![],
            common_ports: vec![4000],
            build_env,
            runtime_copy,
            runtime_env,
            runtime_workdir: None,
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        // Only cache deps/, not _build/
        // Reason: Mix uses filesystem mtimes for incremental compilation.
        // With SOURCE_DATE_EPOCH=0, source files get timestamp 0, but cached
        // _build/ artifacts keep their original timestamps, causing Mix to
        // incorrectly skip recompilation. Caching only deps/ is safe since
        // dependencies rarely change, while allowing clean recompilation.
        vec!["deps".to_string()]
    }

    fn parse_package_metadata(&self, manifest_content: &str) -> Result<(String, bool)> {
        let app_name = Regex::new(r"app:\s*:(\w+)")
            .ok()
            .and_then(|re| re.captures(manifest_content))
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Ok((app_name, true))
    }
}

//! Mix build system (Elixir)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
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

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "elixir:1.15".to_string(),
            runtime_image: "elixir:1.15-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec![
                "mix local.hex --force".to_string(),
                "mix local.rebar --force".to_string(),
                "mix deps.get".to_string(),
                "mix compile".to_string(),
            ],
            cache_paths: vec!["_build/".to_string(), "deps/".to_string()],
            artifacts: vec![],
            common_ports: vec![4000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["_build".to_string(), "deps".to_string()]
    }
}

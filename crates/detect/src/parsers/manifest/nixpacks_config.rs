use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const CONFIG: LanguageId = LanguageId::new("config");
const CONFIG_BS: BuildSystemId = BuildSystemId::new("config");
const CONFIG_RT: RuntimeId = RuntimeId::new("config");

inventory::submit! {
    LanguageMeta { slug: "config", display_name: "Config", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "config", display_name: "Config", aliases: &[] }
}
inventory::submit! {
    RuntimeMeta { slug: "config", display_name: "Config", aliases: &[] }
}

/// Parses nixpacks.toml / nixpacks.json / railpack.json config files
/// that define build plans without a language-specific manifest.
pub struct NixpacksConfigParser;

impl ManifestParser for NixpacksConfigParser {
    fn filenames(&self) -> &[&str] {
        &["nixpacks.toml", "nixpacks.json", "railpack.json"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let filename = path.file_name()?.to_str()?;

        // Don't match if there are other manifests in the same directory
        // (this should only trigger for config-only projects)
        if let Some(parent) = path.parent() {
            if parent.is_absolute() {
                let known_manifests = [
                    "package.json",
                    "Cargo.toml",
                    "go.mod",
                    "pom.xml",
                    "build.gradle",
                    "build.gradle.kts",
                    "pyproject.toml",
                    "requirements.txt",
                    "Gemfile",
                    "composer.json",
                    "mix.exs",
                    "deno.json",
                    "deno.jsonc",
                    "gleam.toml",
                    "shard.yml",
                    "pubspec.yaml",
                    "Package.swift",
                    "deps.edn",
                    "project.clj",
                ];
                for m in &known_manifests {
                    if parent.join(m).exists() {
                        return None;
                    }
                }
            }
        }

        match filename {
            "nixpacks.toml" => parse_nixpacks_toml(path, content),
            "nixpacks.json" => parse_nixpacks_json(path, content),
            "railpack.json" => parse_railpack_json(path, content),
            _ => None,
        }
    }
}

fn parse_nixpacks_toml(path: &Path, content: &str) -> Option<Manifest> {
    let table: toml::Value = toml::from_str(content).ok()?;
    let start_cmd = table
        .get("start")
        .and_then(|s| s.get("cmd"))
        .and_then(|c| c.as_str())?;

    Some(build_config_manifest(path, start_cmd))
}

fn parse_nixpacks_json(path: &Path, content: &str) -> Option<Manifest> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let start_cmd = json
        .get("start")
        .and_then(|s| s.get("cmd"))
        .and_then(|c| c.as_str())?;

    Some(build_config_manifest(path, start_cmd))
}

fn parse_railpack_json(path: &Path, content: &str) -> Option<Manifest> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let start_cmd = json
        .get("deploy")
        .and_then(|d| d.get("startCommand"))
        .and_then(|c| c.as_str())?;

    Some(build_config_manifest(path, start_cmd))
}

fn build_config_manifest(path: &Path, start_cmd: &str) -> Manifest {
    Manifest {
        path: path.to_path_buf(),
        language: CONFIG,
        build_system: CONFIG_BS,
        runtime: CONFIG_RT,
        package: Some(Package {
            name: "app".to_string(),
            version: None,
            is_application: true,
        }),
        workspace: None,
        dependencies: Vec::new(),
        build: BuildSpec {
            packages: vec!["ca-certificates".into()],
            commands: vec!["true".into()],
            member_transform: None,
            env: btree(&[]),
            cache_dirs: vec![],
            artifacts: vec![(".".into(), "/app/".into())],
            setup_commands: vec![],
            build_image: None,
        },
        runtime_config: RuntimeSpec {
            packages: vec!["busybox".into(), "bash".into(), "ca-certificates".into()],
            env: btree(&[]),
            entrypoint: Some(start_cmd.to_string()),
            workdir: Some("/app".into()),
            ports: vec![],
            health_endpoint: None,
        },
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(NixpacksConfigParser))
}

//! .NET build system (C#, F#, VB)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct DotNetBuildSystem;

impl BuildSystem for DotNetBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::DotNet
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "*.csproj".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "*.fsproj".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "*.vbproj".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "*.sln".to_string(),
                priority: 8,
            },
        ]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            let is_dotnet = filename.ends_with(".csproj")
                || filename.ends_with(".fsproj")
                || filename.ends_with(".vbproj")
                || filename.ends_with(".sln");

            if !is_dotnet {
                continue;
            }

            let abs_path = repo_root.join(rel_path);
            let content = fs.read_to_string(&abs_path).ok();

            let is_valid = if let Some(c) = content.as_deref() {
                c.contains("<Project") || c.contains("Microsoft.NET.Sdk")
            } else {
                true
            };

            if is_valid {
                detections.push(DetectionStack::new(
                    BuildSystemId::DotNet,
                    LanguageId::CSharp,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        _service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let dotnet_version = manifest_content
            .and_then(|c| parse_dotnet_version(c))
            .or_else(|| wolfi_index.get_latest_version("dotnet"))
            .expect("Failed to get dotnet version from Wolfi index");

        let _runtime_version = format!("{}-runtime", dotnet_version);

        BuildTemplate {
            build_packages: vec![dotnet_version],
            build_commands: vec![
                "dotnet restore".to_string(),
                "dotnet publish -c Release -o out".to_string(),
            ],
            cache_paths: vec!["/root/.nuget/packages/".to_string(), "obj/".to_string()],
            artifacts: vec!["out/".to_string()],
            common_ports: vec![8080, 5000],
            build_env: std::collections::HashMap::new(),
            runtime_copy: vec![],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![
            ".nuget/packages".to_string(),
            "bin".to_string(),
            "obj".to_string(),
        ]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("Project(")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["*.sln".to_string()]
    }

    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        let mut patterns = Vec::new();

        for line in manifest_content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("Project(") {
                let parts: Vec<&str> = trimmed.split('"').collect();

                if parts.len() >= 4 {
                    let project_path = parts[3];

                    if project_path.ends_with(".csproj")
                        || project_path.ends_with(".fsproj")
                        || project_path.ends_with(".vbproj")
                    {
                        let normalized = project_path.replace('\\', "/");
                        if let Some(dir) = Path::new(&normalized).parent() {
                            patterns.push(dir.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        Ok(patterns)
    }
}

fn parse_dotnet_version(manifest_content: &str) -> Option<String> {
    for line in manifest_content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("<TargetFramework>") {
            if let Some(framework) = trimmed
                .strip_prefix("<TargetFramework>")?
                .strip_suffix("</TargetFramework>")
            {
                if framework.starts_with("net") {
                    let version = framework.trim_start_matches("net");
                    if let Some(major) = version.chars().next() {
                        return Some(format!("dotnet-{}", major));
                    }
                }
            }
        }
    }

    None
}

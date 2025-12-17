//! .NET build system (C#, F#, VB)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct DotNetBuildSystem;

impl BuildSystem for DotNetBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::DotNet
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "*.csproj",
                priority: 10,
            },
            ManifestPattern {
                filename: "*.fsproj",
                priority: 10,
            },
            ManifestPattern {
                filename: "*.vbproj",
                priority: 10,
            },
            ManifestPattern {
                filename: "*.sln",
                priority: 8,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        let is_dotnet = manifest_name.ends_with(".csproj")
            || manifest_name.ends_with(".fsproj")
            || manifest_name.ends_with(".vbproj")
            || manifest_name.ends_with(".sln");

        if !is_dotnet {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("<Project") || content.contains("Microsoft.NET.Sdk")
        } else {
            true
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "mcr.microsoft.com/dotnet/sdk:8.0".to_string(),
            runtime_image: "mcr.microsoft.com/dotnet/aspnet:8.0".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec![
                "dotnet restore".to_string(),
                "dotnet publish -c Release -o out".to_string(),
            ],
            cache_paths: vec!["/root/.nuget/packages/".to_string(), "obj/".to_string()],
            artifacts: vec!["out/".to_string()],
            common_ports: vec![8080, 5000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".nuget/packages".to_string(), "bin".to_string(), "obj".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("Project(")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> &[&str] {
        &["*.sln"]
    }
}

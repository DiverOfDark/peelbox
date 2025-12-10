//! .NET language definition (C#, F#, VB)

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
use regex::Regex;

pub struct DotNetLanguage;

impl LanguageDefinition for DotNetLanguage {
    fn name(&self) -> &str {
        ".NET"
    }

    fn extensions(&self) -> &[&str] {
        &["cs", "fs", "vb"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "*.csproj",
                build_system: "dotnet",
                priority: 10,
            },
            ManifestPattern {
                filename: "*.fsproj",
                build_system: "dotnet",
                priority: 10,
            },
            ManifestPattern {
                filename: "*.vbproj",
                build_system: "dotnet",
                priority: 10,
            },
            ManifestPattern {
                filename: "*.sln",
                build_system: "dotnet",
                priority: 8,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        let is_dotnet = manifest_name.ends_with(".csproj")
            || manifest_name.ends_with(".fsproj")
            || manifest_name.ends_with(".vbproj")
            || manifest_name.ends_with(".sln");

        if !is_dotnet {
            return None;
        }

        let mut confidence = 0.9;
        if let Some(content) = manifest_content {
            if content.contains("<Project") || content.contains("Microsoft.NET.Sdk") {
                confidence = 1.0;
            }
        }

        Some(DetectionResult {
            build_system: "dotnet".to_string(),
            confidence,
        })
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "dotnet" {
            return None;
        }

        Some(BuildTemplate {
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
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["dotnet"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &["bin", "obj", ".nuget"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // <TargetFramework>net8.0</TargetFramework>
        if let Some(caps) = Regex::new(r"<TargetFramework>net(\d+\.\d+)</TargetFramework>")
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // <TargetFramework>net8.0-windows</TargetFramework>
        if let Some(caps) = Regex::new(r"<TargetFramework>net(\d+\.\d+)-")
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // <TargetFramework>netcoreapp3.1</TargetFramework>
        if let Some(caps) = Regex::new(r"<TargetFramework>netcoreapp(\d+\.\d+)</TargetFramework>")
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = DotNetLanguage;
        assert_eq!(lang.name(), ".NET");
    }

    #[test]
    fn test_extensions() {
        let lang = DotNetLanguage;
        assert!(lang.extensions().contains(&"cs"));
        assert!(lang.extensions().contains(&"fs"));
    }

    #[test]
    fn test_detect_csproj() {
        let lang = DotNetLanguage;
        let result = lang.detect("MyApp.csproj", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "dotnet");
    }

    #[test]
    fn test_detect_fsproj() {
        let lang = DotNetLanguage;
        let result = lang.detect("MyApp.fsproj", None);
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_sln() {
        let lang = DotNetLanguage;
        let result = lang.detect("Solution.sln", None);
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_with_content() {
        let lang = DotNetLanguage;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk.Web"></Project>"#;
        let result = lang.detect("MyApp.csproj", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_build_template() {
        let lang = DotNetLanguage;
        let template = lang.build_template("dotnet");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("dotnet/sdk"));
        assert!(t.runtime_image.contains("dotnet/aspnet"));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = DotNetLanguage;
        assert!(lang.excluded_dirs().contains(&"bin"));
        assert!(lang.excluded_dirs().contains(&"obj"));
        assert!(lang.excluded_dirs().contains(&".nuget"));
    }

    #[test]
    fn test_detect_version_net8() {
        let lang = DotNetLanguage;
        let content = r#"<Project><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>"#;
        assert_eq!(lang.detect_version(Some(content)), Some("8.0".to_string()));
    }

    #[test]
    fn test_detect_version_netcoreapp() {
        let lang = DotNetLanguage;
        let content = r#"<Project><PropertyGroup><TargetFramework>netcoreapp3.1</TargetFramework></PropertyGroup></Project>"#;
        assert_eq!(lang.detect_version(Some(content)), Some("3.1".to_string()));
    }
}

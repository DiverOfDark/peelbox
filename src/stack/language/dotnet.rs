//! .NET language definition (C#, F#, VB)

use super::{Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition};
use regex::Regex;

pub struct DotNetLanguage;

impl LanguageDefinition for DotNetLanguage {
    fn id(&self) -> crate::stack::LanguageId {
        crate::stack::LanguageId::CSharp
    }

    fn extensions(&self) -> Vec<String> {
        vec!["cs".to_string(), "fs".to_string(), "vb".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
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
            build_system: crate::stack::BuildSystemId::DotNet,
            confidence,
        })
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["dotnet".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["bin".to_string(), "obj".to_string(), ".nuget".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
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

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let mut external_deps = Vec::new();
        let mut internal_deps = Vec::new();

        if let Ok(re) = Regex::new(r#"<PackageReference\s+Include="([^"]+)"\s+Version="([^"]+)""#) {
            for cap in re.captures_iter(manifest_content) {
                if let (Some(name), Some(version)) = (cap.get(1), cap.get(2)) {
                    external_deps.push(Dependency {
                        name: name.as_str().to_string(),
                        version: Some(version.as_str().to_string()),
                        is_internal: false,
                    });
                }
            }
        }

        if let Ok(re) = Regex::new(r#"<ProjectReference\s+Include="([^"]+)""#) {
            for cap in re.captures_iter(manifest_content) {
                if let Some(path_match) = cap.get(1) {
                    let path_str = path_match.as_str();
                    let is_internal = all_internal_paths
                        .iter()
                        .any(|p| p.to_str().is_some_and(|s| s.contains(path_str)));

                    let name = std::path::Path::new(path_str)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(path_str)
                        .to_string();

                    if is_internal {
                        internal_deps.push(Dependency {
                            name,
                            version: None,
                            is_internal: true,
                        });
                    } else {
                        external_deps.push(Dependency {
                            name,
                            version: None,
                            is_internal: false,
                        });
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"Environment\.GetEnvironmentVariable\("([A-Z_][A-Z0-9_]*)""#.to_string(),
            "Environment.GetEnvironmentVariable".to_string(),
        )]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r#"UseUrls\([^:)]*:(\d{4,5})"#.to_string(), "UseUrls()".to_string()),
            (
                r#"ApplicationUrl['"]\s*=\s*[^:]*:(\d{4,5})"#.to_string(),
                "ApplicationUrl".to_string(),
            ),
        ]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![(r#"MapGet\(['"]([/\w\-]*health[/\w\-]*)['"]"#.to_string(), "ASP.NET".to_string())]
    }

    fn default_health_endpoints(&self) -> Vec<(String, String)> {
        vec![("/health".to_string(), "ASP.NET Core".to_string())]
    }

    fn default_env_vars(&self) -> Vec<String> {
        vec![]
    }

    fn is_main_file(&self, fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name == "Program.cs" {
                return true;
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("static void Main") || content.contains("static async Task Main") {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<String> {
        Some("dotnet".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(8080)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("dotnet run".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = DotNetLanguage;
        assert!(lang.extensions().iter().any(|s| s == "cs"));
        assert!(lang.extensions().iter().any(|s| s == "fs"));
    }

    #[test]
    fn test_detect_csproj() {
        let lang = DotNetLanguage;
        let result = lang.detect("MyApp.csproj", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::DotNet);
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
    fn test_compatible_build_systems() {
        let lang = DotNetLanguage;
        assert_eq!(lang.compatible_build_systems(), vec!["dotnet".to_string()]);
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = DotNetLanguage;
        assert!(lang.excluded_dirs().iter().any(|s| s == "bin"));
        assert!(lang.excluded_dirs().iter().any(|s| s == "obj"));
        assert!(lang.excluded_dirs().iter().any(|s| s == ".nuget"));
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

    #[test]
    fn test_parse_dependencies_package_references() {
        let lang = DotNetLanguage;
        let content = r#"
<Project Sdk="Microsoft.NET.Sdk.Web">
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Serilog" Version="2.12.0" />
  </ItemGroup>
</Project>
"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "Newtonsoft.Json" && d.version == Some("13.0.3".to_string())));
        assert!(deps.external_deps.iter().any(|d| d.name == "Serilog"));
    }

    #[test]
    fn test_parse_dependencies_project_references() {
        let lang = DotNetLanguage;
        let content = r#"
<Project>
  <ItemGroup>
    <ProjectReference Include="../MyLib/MyLib.csproj" />
    <ProjectReference Include="../AnotherLib/AnotherLib.csproj" />
  </ItemGroup>
</Project>
"#;
        let internal_paths = vec![std::path::PathBuf::from("../MyLib/MyLib.csproj")];
        let deps = lang.parse_dependencies(content, &internal_paths);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.external_deps.len(), 1);
        assert!(deps
            .internal_deps
            .iter()
            .any(|d| d.name == "MyLib" && d.is_internal));
    }

    #[test]
    fn test_parse_dependencies_empty() {
        let lang = DotNetLanguage;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk"></Project>"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.is_empty());
        assert!(deps.internal_deps.is_empty());
    }
}

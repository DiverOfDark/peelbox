use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct DotNetRuntime;

impl DotNetRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let env_pattern =
            Regex::new(r#"Environment\.GetEnvironmentVariable\("([A-Z_][A-Z0-9_]*)"\)"#).unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "cs" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        for cap in env_pattern.captures_iter(&content) {
                            if let Some(var) = cap.get(1) {
                                env_vars.insert(var.as_str().to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut vars: Vec<String> = env_vars.into_iter().collect();
        vars.sort();
        vars
    }

    fn extract_ports(&self, files: &[PathBuf]) -> Option<u16> {
        for file in files {
            if file
                .file_name()
                .is_some_and(|n| n == "launchSettings.json")
            {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        if let Some(profiles) = json["profiles"].as_object() {
                            for profile in profiles.values() {
                                if let Some(url) = profile["applicationUrl"].as_str() {
                                    let port_re = Regex::new(r":(\d+)").unwrap();
                                    if let Some(cap) = port_re.captures(url) {
                                        if let Some(port_str) = cap.get(1) {
                                            if let Ok(port) = port_str.as_str().parse::<u16>() {
                                                return Some(port);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_native_deps(&self, files: &[PathBuf]) -> Vec<String> {
        let mut deps = HashSet::new();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "csproj" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if content.contains("<NativeLibrary")
                            || content.contains("Interop")
                            || content.contains("PInvoke")
                        {
                            deps.insert("build-base".to_string());
                        }
                    }
                }
            }
        }

        let mut result: Vec<String> = deps.into_iter().collect();
        result.sort();
        result
    }

    fn extract_entrypoint(&self, files: &[PathBuf]) -> Option<String> {
        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "csproj" || ext == "fsproj" || ext == "vbproj" {
                    let assembly_name = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("app");
                    return Some(format!("dotnet /app/{}.dll", assembly_name));
                }
            }
        }
        None
    }
}

impl Runtime for DotNetRuntime {
    fn name(&self) -> &str {
        ".NET"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let env_vars = self.extract_env_vars(files);
        let native_deps = self.extract_native_deps(files);
        let detected_port = self.extract_ports(files);
        let entrypoint = self.extract_entrypoint(files);

        let port =
            detected_port.or_else(|| framework.and_then(|f| f.default_ports().first().copied()));
        let health = framework.and_then(|f| {
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint,
            port,
            env_vars,
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("8.0");
        format!("mcr.microsoft.com/dotnet/aspnet:{}", version)
    }

    fn required_packages(&self) -> Vec<String> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("dotnet {}", entrypoint.display())
    }

    fn runtime_packages(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> Vec<String> {
        let requested = self.detect_version(service_path, manifest_content);
        let available = wolfi_index.get_versions("dotnet");

        let base_version = requested
            .as_deref()
            .and_then(|r| wolfi_index.match_version("dotnet", r, &available))
            .or_else(|| wolfi_index.get_latest_version("dotnet"))
            .unwrap_or_else(|| "dotnet-8".to_string());

        // Check if this is an ASP.NET Core application
        let is_aspnet = manifest_content
            .map(|content| content.contains("Microsoft.NET.Sdk.Web") || content.contains("Microsoft.AspNetCore"))
            .unwrap_or(false);

        let package_prefix = if is_aspnet { "aspnet" } else { "dotnet" };
        let version = format!("{}-{}-runtime", package_prefix, base_version.trim_start_matches("dotnet-"));
        vec![version]
    }
}

impl DotNetRuntime {
    fn detect_version(&self, service_path: &Path, manifest_content: Option<&str>) -> Option<String> {
        if let Some(content) = manifest_content {
            for csproj in ["*.csproj", "*.fsproj", "*.vbproj"] {
                let pattern_path = service_path.join(csproj);
                if pattern_path.parent().map(|p| p.exists()).unwrap_or(false) {
                    return self.parse_target_framework(content);
                }
            }
        }
        None
    }

    fn parse_target_framework(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<TargetFramework>") {
                if let Some(framework) = trimmed
                    .strip_prefix("<TargetFramework>")?
                    .strip_suffix("</TargetFramework>")
                {
                    if framework.starts_with("net") {
                        let version = framework.trim_start_matches("net");
                        if let Some(major) = version.chars().next() {
                            return Some(major.to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_dotnet_runtime_name() {
        let runtime = DotNetRuntime;
        assert_eq!(runtime.name(), ".NET");
    }

    #[test]
    fn test_dotnet_runtime_base_image_default() {
        let runtime = DotNetRuntime;
        assert_eq!(
            runtime.runtime_base_image(None),
            "mcr.microsoft.com/dotnet/aspnet:8.0"
        );
    }

    #[test]
    fn test_dotnet_runtime_base_image_versioned() {
        let runtime = DotNetRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("7.0")),
            "mcr.microsoft.com/dotnet/aspnet:7.0"
        );
    }

    #[test]
    fn test_dotnet_required_packages() {
        let runtime = DotNetRuntime;
        let packages: Vec<String> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_dotnet_start_command() {
        let runtime = DotNetRuntime;
        let entrypoint = Path::new("app.dll");
        assert_eq!(runtime.start_command(entrypoint), "dotnet app.dll");
    }

    #[test]
    fn test_extract_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let cs_file = temp_dir.path().join("Program.cs");
        fs::write(
            &cs_file,
            r#"
using System;
var dbUrl = Environment.GetEnvironmentVariable("DATABASE_URL");
var apiKey = Environment.GetEnvironmentVariable("API_KEY");
"#,
        )
        .unwrap();

        let runtime = DotNetRuntime;
        let files = vec![cs_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL"]);
    }

    #[test]
    fn test_extract_ports_launch_settings() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("launchSettings.json");
        fs::write(
            &settings_file,
            r#"
{
  "profiles": {
    "http": {
      "applicationUrl": "http://localhost:5000"
    }
  }
}
"#,
        )
        .unwrap();

        let runtime = DotNetRuntime;
        let files = vec![settings_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(5000));
    }

    #[test]
    fn test_extract_native_deps() {
        let temp_dir = TempDir::new().unwrap();
        let csproj_file = temp_dir.path().join("App.csproj");
        fs::write(
            &csproj_file,
            r#"
<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <NativeLibrary Include="native.so" />
  </ItemGroup>
</Project>
"#,
        )
        .unwrap();

        let runtime = DotNetRuntime;
        let files = vec![csproj_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base".to_string()]);
    }
}

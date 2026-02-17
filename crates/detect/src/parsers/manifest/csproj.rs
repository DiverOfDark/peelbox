use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const CSHARP: LanguageId = LanguageId::new("csharp");
const FSHARP: LanguageId = LanguageId::new("fsharp");
const DOTNET_BS: BuildSystemId = BuildSystemId::new("dotnet");
const DOTNET_RT: RuntimeId = RuntimeId::new("dotnet");

inventory::submit! {
    LanguageMeta { slug: "csharp", display_name: "C#", aliases: &[] }
}
inventory::submit! {
    LanguageMeta { slug: "fsharp", display_name: "F#", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "dotnet", display_name: ".NET", aliases: &["dotnet"] }
}
inventory::submit! {
    RuntimeMeta { slug: "dotnet", display_name: ".NET", aliases: &["dotnet", "csharp", "fsharp"] }
}

pub struct CsprojParser;

impl ManifestParser for CsprojParser {
    fn filenames(&self) -> &[&str] {
        // Matched by extension in pipeline.rs classify_file
        &[]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("<Project") {
            return None;
        }

        let is_fsharp = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "fsproj")
            .unwrap_or(false);

        let language = if is_fsharp { FSHARP } else { CSHARP };

        let file_stem = path.file_stem().and_then(|s| s.to_str()).map(String::from);

        // Extract .NET version from TargetFramework (e.g., "net8.0" -> "8")
        let dotnet_version = regex::Regex::new(r"<TargetFramework>net(\d+)\.\d+</TargetFramework>")
            .ok()
            .and_then(|re| {
                re.captures(content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            });

        let sdk_pkg = dotnet_version
            .as_ref()
            .map(|v| format!("dotnet-{}-sdk", v))
            .unwrap_or_else(|| "dotnet-sdk".into());

        let runtime_pkg = dotnet_version
            .as_ref()
            .map(|v| format!("aspnet-{}-runtime", v))
            .unwrap_or_else(|| "dotnet-runtime".into());

        // Parse dependencies from PackageReference elements
        let dependencies = parse_csproj_deps(content);

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: DOTNET_BS,
            runtime: DOTNET_RT,
            package: Some(Package {
                name: "app".to_string(), // Normalized to "app"
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![sdk_pkg, "ca-certificates".into()],
                commands: vec![
                    "dotnet restore".into(),
                    "dotnet publish -c Release -o out".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("DOTNET_CLI_HOME", "/root"),
                    ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
                    ("DOTNET_NOLOGO", "1"),
                    ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
                ]),
                cache_dirs: vec![".nuget/packages".into(), "bin".into(), "obj".into()],
                artifacts: vec![("out/".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![runtime_pkg, "ca-certificates".into()],
                env: BTreeMap::new(), // ASPNETCORE_URLS set by framework detector
                entrypoint: file_stem.map(|n| format!("dotnet /app/{}.dll", n)),
                workdir: Some("/app".into()),
                ports: vec![5000],
                health_endpoint: None,
            },
        })
    }
}

fn parse_csproj_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let re = regex::Regex::new(r#"<PackageReference\s+Include="([^"]+)""#).unwrap();
    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            deps.push(Dependency {
                name: m.as_str().to_string(),
                version: None,
                scope: DepScope::Runtime,
                is_internal: false,
            });
        }
    }
    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(CsprojParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_parse_csproj() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Microsoft.AspNetCore.OpenApi" Version="8.0.0" />
  </ItemGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("App.csproj"), content).unwrap();
        assert_eq!(manifest.language, CSHARP);
        assert_eq!(manifest.build_system, DOTNET_BS);
        assert_eq!(manifest.runtime, DOTNET_RT);
        assert_eq!(
            manifest.build.packages,
            vec!["dotnet-8-sdk", "ca-certificates"]
        );
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["aspnet-8-runtime", "ca-certificates"]
        );
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("dotnet /app/App.dll".into())
        );
        assert_eq!(manifest.dependencies.len(), 1);
        assert_eq!(
            manifest.dependencies[0].name,
            "Microsoft.AspNetCore.OpenApi"
        );
    }

    #[test]
    fn test_parse_fsproj() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Microsoft.AspNetCore.OpenApi" Version="8.0.0" />
  </ItemGroup>
</Project>"#;

        let manifest = parser
            .parse(Path::new("FSharpApi.fsproj"), content)
            .unwrap();
        assert_eq!(manifest.language, FSHARP);
        assert_eq!(manifest.build_system, DOTNET_BS);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("dotnet /app/FSharpApi.dll".into())
        );
        assert_eq!(manifest.dependencies.len(), 1);
    }

    #[test]
    fn test_parse_fsproj_cli() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser
            .parse(Path::new("FSharpCli.fsproj"), content)
            .unwrap();
        assert_eq!(manifest.language, FSHARP);
        assert_eq!(manifest.dependencies.len(), 0);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("dotnet /app/FSharpCli.dll".into())
        );
    }

    #[test]
    fn test_rejects_non_project_content() {
        let parser = CsprojParser;
        let result = parser.parse(Path::new("Readme.fsproj"), "Just some text");
        assert!(result.is_none());
    }

    #[test]
    fn test_fallback_dotnet_version() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>netstandard2.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("Lib.csproj"), content).unwrap();
        assert_eq!(
            manifest.build.packages,
            vec!["dotnet-sdk", "ca-certificates"]
        );
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["dotnet-runtime", "ca-certificates"]
        );
    }
}

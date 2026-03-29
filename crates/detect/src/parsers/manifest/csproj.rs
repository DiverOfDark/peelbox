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

/// Minimum .NET major version natively available as a Wolfi package.
/// Older versions still use Wolfi packages (resolved to latest by Wolfi resolution)
/// with DOTNET_ROLL_FORWARD=LatestMajor for runtime compatibility.
const MIN_WOLFI_DOTNET_NATIVE: u32 = 8;

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

/// Parsed .NET version info from TargetFramework.
struct DotnetVersion {
    /// Major version number (e.g., 6, 8, 9).
    major: u32,
}

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

        // Extract .NET version from TargetFramework (e.g., "net8.0" -> major=8, channel="8.0")
        let dotnet_version = parse_dotnet_version(content);

        // Parse dependencies from PackageReference elements
        let dependencies = parse_csproj_deps(content);

        // Detect if this is a web project (Microsoft.NET.Sdk.Web) vs a CLI/library
        let is_web = content.contains("Microsoft.NET.Sdk.Web");

        let (build, runtime_config) = if let Some(ref ver) = dotnet_version {
            build_wolfi_dotnet_specs(ver, &file_stem, is_web)
        } else {
            build_fallback_dotnet_specs(&file_stem, is_web)
        };

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
            build,
            runtime_config,
        })
    }
}

/// Parse the .NET version from a TargetFramework element.
fn parse_dotnet_version(content: &str) -> Option<DotnetVersion> {
    let re = regex::Regex::new(r"<TargetFramework>net(\d+)\.\d+</TargetFramework>").ok()?;
    let caps = re.captures(content)?;
    let major = caps.get(1)?.as_str().parse::<u32>().ok()?;
    Some(DotnetVersion { major })
}

/// Build specs using Wolfi packages.
/// For old versions not natively available in Wolfi, the package names (e.g.,
/// `dotnet-6-sdk`) are resolved to the latest available version by Wolfi
/// resolution. `DOTNET_ROLL_FORWARD=LatestMajor` ensures the app runs on the
/// newer runtime.
fn build_wolfi_dotnet_specs(
    ver: &DotnetVersion,
    file_stem: &Option<String>,
    is_web: bool,
) -> (BuildSpec, RuntimeSpec) {
    let sdk_pkg = format!("dotnet-{}-sdk", ver.major);
    let runtime_pkg = format!("aspnet-{}-runtime", ver.major);

    // For old versions that will be upgraded by Wolfi resolution,
    // enable roll-forward so the app runs on the newer runtime.
    let runtime_env = if ver.major < MIN_WOLFI_DOTNET_NATIVE {
        let mut env = BTreeMap::new();
        env.insert(
            "DOTNET_ROLL_FORWARD".into(),
            "LatestMajor".into(),
        );
        env
    } else {
        BTreeMap::new() // ASPNETCORE_URLS set by framework detector
    };

    // Only assign a default port for web projects (Microsoft.NET.Sdk.Web).
    // CLI apps (Microsoft.NET.Sdk) don't listen on a port.
    let ports = if is_web { vec![5000] } else { vec![] };

    (
        BuildSpec {
            packages: vec![sdk_pkg, "ca-certificates".into()],
            commands: dotnet_build_commands(),
            member_transform: None,
            env: dotnet_build_env(),
            cache_dirs: dotnet_cache_dirs(),
            artifacts: vec![("out/".into(), "/app".into())],
                    build_image: None,
},
        RuntimeSpec {
            packages: vec![runtime_pkg, "ca-certificates".into()],
            env: runtime_env,
            entrypoint: file_stem.as_ref().map(|n| format!("dotnet /app/{}.dll", n)),
            workdir: Some("/app".into()),
            ports,
            health_endpoint: None,
        },
    )
}

/// Build specs when no .NET version could be parsed (fallback).
fn build_fallback_dotnet_specs(file_stem: &Option<String>, is_web: bool) -> (BuildSpec, RuntimeSpec) {
    let ports = if is_web { vec![5000] } else { vec![] };

    (
        BuildSpec {
            packages: vec!["dotnet-sdk".into(), "ca-certificates".into()],
            commands: dotnet_build_commands(),
            member_transform: None,
            env: dotnet_build_env(),
            cache_dirs: dotnet_cache_dirs(),
            artifacts: vec![("out/".into(), "/app".into())],
                    build_image: None,
},
        RuntimeSpec {
            packages: vec!["dotnet-runtime".into(), "ca-certificates".into()],
            env: BTreeMap::new(),
            entrypoint: file_stem.as_ref().map(|n| format!("dotnet /app/{}.dll", n)),
            workdir: Some("/app".into()),
            ports,
            health_endpoint: None,
        },
    )
}

/// Common .NET build commands.
/// Removes global.json (if present) to avoid SDK version pinning conflicts,
/// then restores and publishes.
fn dotnet_build_commands() -> Vec<String> {
    vec![
        "rm -f global.json".into(),
        "dotnet restore".into(),
        "dotnet publish -c Release -o out".into(),
    ]
}

/// Common .NET build environment variables.
fn dotnet_build_env() -> BTreeMap<String, String> {
    btree(&[
        ("DOTNET_CLI_HOME", "/root"),
        ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
        ("DOTNET_NOLOGO", "1"),
        ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
    ])
}

/// Common .NET cache directories.
fn dotnet_cache_dirs() -> Vec<String> {
    vec![".nuget/packages".into(), "bin".into(), "obj".into()]
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

    #[test]
    fn test_dotnet6_uses_wolfi_with_roll_forward() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("App.csproj"), content).unwrap();

        // Old .NET versions now use Wolfi packages (resolved to latest by Wolfi).
        assert_eq!(
            manifest.build.packages,
            vec!["dotnet-6-sdk", "ca-certificates"]
        );
        assert_eq!(manifest.build.commands.len(), 3);
        assert_eq!(manifest.build.commands[0], "rm -f global.json");
        assert_eq!(manifest.build.commands[1], "dotnet restore");
        assert_eq!(
            manifest.build.commands[2],
            "dotnet publish -c Release -o out"
        );

        // Build env should NOT have DOTNET_ROOT
        assert!(!manifest.build.env.contains_key("DOTNET_ROOT"));

        // Runtime should use Wolfi packages
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["aspnet-6-runtime", "ca-certificates"]
        );

        // Runtime env should have DOTNET_ROLL_FORWARD for old versions
        assert_eq!(
            manifest.runtime_config.env.get("DOTNET_ROLL_FORWARD"),
            Some(&"LatestMajor".to_string())
        );
    }

    #[test]
    fn test_dotnet7_uses_wolfi_with_roll_forward() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net7.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("MyApp.fsproj"), content).unwrap();

        // .NET 7 should also use Wolfi packages
        assert_eq!(
            manifest.build.packages,
            vec!["dotnet-7-sdk", "ca-certificates"]
        );
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["aspnet-7-runtime", "ca-certificates"]
        );
        assert_eq!(
            manifest.runtime_config.env.get("DOTNET_ROLL_FORWARD"),
            Some(&"LatestMajor".to_string())
        );
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("dotnet /app/MyApp.dll".into())
        );
    }

    #[test]
    fn test_dotnet9_uses_wolfi() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net9.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("App.csproj"), content).unwrap();

        // .NET 9 should use Wolfi packages (>= MIN_WOLFI_DOTNET_VERSION)
        assert_eq!(
            manifest.build.packages,
            vec!["dotnet-9-sdk", "ca-certificates"]
        );
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["aspnet-9-runtime", "ca-certificates"]
        );
        // Should NOT have DOTNET_ROOT in env
        assert!(!manifest.build.env.contains_key("DOTNET_ROOT"));
    }
}

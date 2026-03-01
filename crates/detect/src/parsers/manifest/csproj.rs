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

/// Minimum .NET major version available as a Wolfi package.
/// Versions below this threshold use Microsoft's dotnet-install.sh script instead.
const MIN_WOLFI_DOTNET_VERSION: u32 = 8;

/// Directory where the .NET SDK/runtime is installed when using dotnet-install.sh.
const DOTNET_INSTALL_DIR: &str = "/usr/share/dotnet";

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
    /// Channel string for dotnet-install.sh (e.g., "6.0", "8.0").
    channel: String,
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

        let (build, runtime_config) = if let Some(ref ver) = dotnet_version {
            if ver.major < MIN_WOLFI_DOTNET_VERSION {
                build_legacy_dotnet_specs(ver, &file_stem)
            } else {
                build_wolfi_dotnet_specs(ver, &file_stem)
            }
        } else {
            build_fallback_dotnet_specs(&file_stem)
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
    let re = regex::Regex::new(r"<TargetFramework>net(\d+)\.(\d+)</TargetFramework>").ok()?;
    let caps = re.captures(content)?;
    let major_str = caps.get(1)?.as_str();
    let minor_str = caps.get(2)?.as_str();
    let major = major_str.parse::<u32>().ok()?;
    Some(DotnetVersion {
        major,
        channel: format!("{}.{}", major_str, minor_str),
    })
}

/// Build specs for .NET versions available as Wolfi packages (>= MIN_WOLFI_DOTNET_VERSION).
fn build_wolfi_dotnet_specs(ver: &DotnetVersion, file_stem: &Option<String>) -> (BuildSpec, RuntimeSpec) {
    let sdk_pkg = format!("dotnet-{}-sdk", ver.major);
    let runtime_pkg = format!("aspnet-{}-runtime", ver.major);

    (
        BuildSpec {
            packages: vec![sdk_pkg, "ca-certificates".into()],
            commands: vec![
                "dotnet restore".into(),
                "dotnet publish -c Release -o out".into(),
            ],
            member_transform: None,
            env: dotnet_build_env(),
            cache_dirs: dotnet_cache_dirs(),
            artifacts: vec![("out/".into(), "/app".into())],
        },
        RuntimeSpec {
            packages: vec![runtime_pkg, "ca-certificates".into()],
            env: BTreeMap::new(), // ASPNETCORE_URLS set by framework detector
            entrypoint: file_stem.as_ref().map(|n| format!("dotnet /app/{}.dll", n)),
            workdir: Some("/app".into()),
            ports: vec![5000],
            health_endpoint: None,
        },
    )
}

/// Build specs for old .NET versions not available in Wolfi (< MIN_WOLFI_DOTNET_VERSION).
/// Uses Microsoft's dotnet-install.sh script to download and install the SDK/runtime.
fn build_legacy_dotnet_specs(ver: &DotnetVersion, file_stem: &Option<String>) -> (BuildSpec, RuntimeSpec) {
    let install_cmd = format!(
        "curl -fsSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel {} --install-dir {}",
        ver.channel, DOTNET_INSTALL_DIR
    );

    let mut build_env = dotnet_build_env();
    build_env.insert("DOTNET_ROOT".into(), DOTNET_INSTALL_DIR.into());
    build_env.insert("PATH".into(), format!("{}:${{PATH}}", DOTNET_INSTALL_DIR));

    let mut runtime_env = BTreeMap::new();
    runtime_env.insert("DOTNET_ROOT".into(), DOTNET_INSTALL_DIR.into());
    runtime_env.insert("PATH".into(), format!("{}:${{PATH}}", DOTNET_INSTALL_DIR));

    (
        BuildSpec {
            packages: vec![
                "build-base".into(),
                "bash".into(),
                "curl".into(),
                "icu".into(),
                "ca-certificates".into(),
            ],
            commands: vec![
                install_cmd,
                "dotnet restore".into(),
                "dotnet publish -c Release -o out".into(),
            ],
            member_transform: None,
            env: build_env,
            cache_dirs: dotnet_cache_dirs(),
            artifacts: vec![("out/".into(), "/app".into())],
        },
        RuntimeSpec {
            packages: vec![
                "icu".into(),
                "ca-certificates".into(),
            ],
            env: runtime_env,
            entrypoint: file_stem.as_ref().map(|n| format!("dotnet /app/{}.dll", n)),
            workdir: Some("/app".into()),
            ports: vec![5000],
            health_endpoint: None,
        },
    )
}

/// Build specs when no .NET version could be parsed (fallback).
fn build_fallback_dotnet_specs(file_stem: &Option<String>) -> (BuildSpec, RuntimeSpec) {
    (
        BuildSpec {
            packages: vec!["dotnet-sdk".into(), "ca-certificates".into()],
            commands: vec![
                "dotnet restore".into(),
                "dotnet publish -c Release -o out".into(),
            ],
            member_transform: None,
            env: dotnet_build_env(),
            cache_dirs: dotnet_cache_dirs(),
            artifacts: vec![("out/".into(), "/app".into())],
        },
        RuntimeSpec {
            packages: vec!["dotnet-runtime".into(), "ca-certificates".into()],
            env: BTreeMap::new(),
            entrypoint: file_stem.as_ref().map(|n| format!("dotnet /app/{}.dll", n)),
            workdir: Some("/app".into()),
            ports: vec![5000],
            health_endpoint: None,
        },
    )
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
    fn test_legacy_dotnet6_csproj() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("App.csproj"), content).unwrap();

        // Build should use dotnet-install.sh instead of Wolfi packages
        assert_eq!(
            manifest.build.packages,
            vec!["build-base", "bash", "curl", "icu", "ca-certificates"]
        );
        assert_eq!(manifest.build.commands.len(), 3);
        assert!(manifest.build.commands[0].contains("dotnet-install.sh"));
        assert!(manifest.build.commands[0].contains("--channel 6.0"));
        assert!(manifest.build.commands[0].contains("--install-dir /usr/share/dotnet"));
        assert_eq!(manifest.build.commands[1], "dotnet restore");
        assert_eq!(manifest.build.commands[2], "dotnet publish -c Release -o out");

        // Build env should include DOTNET_ROOT and PATH
        assert_eq!(
            manifest.build.env.get("DOTNET_ROOT"),
            Some(&"/usr/share/dotnet".to_string())
        );
        assert_eq!(
            manifest.build.env.get("PATH"),
            Some(&"/usr/share/dotnet:${PATH}".to_string())
        );

        // Runtime should not have SDK/runtime Wolfi packages
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["icu", "ca-certificates"]
        );

        // Runtime env should include DOTNET_ROOT and PATH
        assert_eq!(
            manifest.runtime_config.env.get("DOTNET_ROOT"),
            Some(&"/usr/share/dotnet".to_string())
        );
        assert_eq!(
            manifest.runtime_config.env.get("PATH"),
            Some(&"/usr/share/dotnet:${PATH}".to_string())
        );
    }

    #[test]
    fn test_legacy_dotnet7_fsproj() {
        let parser = CsprojParser;
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net7.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let manifest = parser.parse(Path::new("MyApp.fsproj"), content).unwrap();

        // Should use legacy install for .NET 7 (below MIN_WOLFI_DOTNET_VERSION)
        assert!(manifest.build.commands[0].contains("--channel 7.0"));
        assert_eq!(
            manifest.runtime_config.packages,
            vec!["icu", "ca-certificates"]
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

use crate::helpers::btree;
use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_stack::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

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

        let language = if is_fsharp {
            LanguageId::FSharp
        } else {
            LanguageId::CSharp
        };

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from);

        // Extract .NET version from TargetFramework (e.g., "net8.0" -> "8")
        let dotnet_version =
            regex::Regex::new(r"<TargetFramework>net(\d+)\.\d+</TargetFramework>")
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
            build_system: BuildSystemId::DotNet,
            runtime: RuntimeId::DotNet,
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

use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const ELIXIR: LanguageId = LanguageId::new("elixir");
const MIX: BuildSystemId = BuildSystemId::new("mix");
const BEAM: RuntimeId = RuntimeId::new("beam");

inventory::submit! {
    LanguageMeta { slug: "elixir", display_name: "Elixir", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "mix", display_name: "Mix", aliases: &["mix"] }
}
inventory::submit! {
    RuntimeMeta { slug: "beam", display_name: "BEAM", aliases: &["elixir"] }
}

pub struct MixExsParser;

/// Extract dependency names from a mix.exs deps block.
/// Matches patterns like `{:dep_name, "~> 1.0"}` and `{:dep_name, ">= 0.0.0"}`.
fn parse_mix_deps(content: &str) -> Vec<Dependency> {
    let dep_re = match regex::Regex::new(r#"\{:(\w+)\s*,"#) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    // Find the deps function body (match full signature to avoid partial matches)
    let deps_start = match content.find("defp deps do") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    // Find the opening bracket after defp deps do
    let after_deps = &content[deps_start..];
    let bracket_start = match after_deps.find('[') {
        Some(pos) => deps_start + pos,
        None => return Vec::new(),
    };

    // Find matching closing bracket (simple nesting)
    let mut depth = 0;
    let mut bracket_end = None;
    for (i, ch) in content[bracket_start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    bracket_end = Some(bracket_start + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let bracket_end = match bracket_end {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let deps_block = &content[bracket_start..=bracket_end];
    dep_re
        .captures_iter(deps_block)
        .map(|cap| Dependency {
            name: cap[1].to_string(),
            version: None,
            scope: DepScope::Runtime,
            is_internal: false,
        })
        .collect()
}

impl ManifestParser for MixExsParser {
    fn filenames(&self) -> &[&str] {
        &["mix.exs"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("defmodule") || !content.contains("def project") {
            return None;
        }

        let app_re = regex::Regex::new(r#"app:\s*:(\w+)"#).ok()?;
        let name = app_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        // Extract Elixir version from `elixir: "~> 1.14"` or similar
        let elixir_version = parse_elixir_version(content);

        let dependencies = parse_mix_deps(content);

        let has_phoenix = dependencies.iter().any(|d| d.name == "phoenix");
        let has_web = dependencies.iter().any(|d| {
            matches!(
                d.name.as_str(),
                "phoenix" | "plug" | "plug_cowboy" | "bandit"
            )
        });
        let has_assets_dir = path
            .parent()
            .map(|d| d.join("assets").is_dir())
            .unwrap_or(false);

        let mut build_commands = vec![
            "mix local.hex --force && mix local.rebar --force".into(),
            "rm -f mix.lock && mix deps.get".into(),
            // If deps.compile fails (e.g. type conflicts with newer Elixir),
            // widen all version constraints in mix.exs, clean stale artifacts,
            // re-resolve, and retry. Use find+rm instead of rm -rf for dirs
            // that may be mount points (which can't be removed themselves).
            "mix deps.compile || { sed -i 's/\"~> [0-9][^\"]*\"/\">= 0.0.0\"/g' mix.exs && find deps _build -mindepth 1 -delete 2>/dev/null; rm -f mix.lock && mix deps.get && mix deps.compile; }".into(),
        ];
        if has_phoenix && has_assets_dir {
            build_commands.push("mix assets.deploy".into());
        }
        build_commands.push("mix compile".into());

        let mut build_packages = vec![
            "elixir".into(),
            "erlang".into(),
            "erlang-dev".into(),
            "git".into(),
            "build-base".into(),
            "openssl".into(),
            "ca-certificates".into(),
        ];
        if has_phoenix && has_assets_dir {
            build_packages.push("nodejs".into());
            build_packages.push("npm".into());
        }

        // Construct Docker Hub build image from Elixir version (if present)
        let build_image = elixir_version
            .as_ref()
            .map(|v| format!("docker.io/library/elixir:{}", v))
            .unwrap_or_else(|| "docker.io/library/elixir:latest".into());

        Some(Manifest {
            path: path.to_path_buf(),
            language: ELIXIR,
            build_system: MIX,
            runtime: BEAM,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: build_packages,
                commands: build_commands,
                member_transform: None,
                env: btree(&[
                    ("DATABASE_URL", ""),
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/app/.mix"),
                    ("POOL_SIZE", ""),
                    ("SECRET_KEY_BASE", ""),
                ]),
                cache_dirs: vec!["deps".into(), "_build".into()],
                artifacts: vec![(".".into(), "/app".into())],
                build_image: Some(build_image),
                asset_build: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "busybox".into(),
                    "git".into(),
                    "openssl".into(),
                    "ca-certificates".into(),
                ],
                env: btree(&[
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/app/.mix"),
                    ("PORT", "4000"),
                ]),
                entrypoint: Some("mix run --no-halt".into()),
                workdir: Some("/app".into()),
                ports: if has_web { vec![4000] } else { vec![] },
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(MixExsParser))
}

/// Extract Elixir version from `elixir: "~> 1.14"` or `elixir: ">= 1.12.0"` in the project block.
/// Returns the major.minor version string (e.g., "1.14").
fn parse_elixir_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"elixir:\s*["'][~>=<\s]*(\d+\.\d+)"#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["Elixir"],
        extensions: &["ex", "exs"],
        port_patterns: &[r#"port:\s*(\d{4,5})"#],
        health_patterns: &[],
        env_var_patterns: &[r#"System\.get_env\(["']([A-Z_][A-Z0-9_]*)"#],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_parse_mix_deps_extracts_dependencies() {
        let content = r#"
defmodule MyApp.MixProject do
  use Mix.Project

  def project do
    [
      app: :my_app,
      version: "1.0.0",
      deps: deps()
    ]
  end

  defp deps do
    [
      {:ecto, "~> 3.11"},
      {:ecto_sql, "~> 3.11"},
      {:postgrex, "~> 0.18"},
      {:jason, "~> 1.4"}
    ]
  end
end
"#;
        let deps = parse_mix_deps(content);
        let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["ecto", "ecto_sql", "postgrex", "jason"]);
    }

    #[test]
    fn test_parse_mix_deps_empty_when_no_deps_function() {
        let content = r#"
defmodule MyApp.MixProject do
  use Mix.Project
  def project do
    [app: :my_app]
  end
end
"#;
        let deps = parse_mix_deps(content);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_mix_parser_includes_dependencies() {
        let parser = MixExsParser;
        let content = r#"
defmodule EctoApp.MixProject do
  use Mix.Project

  def project do
    [
      app: :ecto_app,
      version: "0.1.0",
      deps: deps()
    ]
  end

  defp deps do
    [
      {:ecto, "~> 3.11"},
      {:phoenix, "~> 1.7"}
    ]
  end
end
"#;
        let manifest = parser.parse(Path::new("mix.exs"), content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "ecto");
        assert_eq!(manifest.dependencies[1].name, "phoenix");
    }
}

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

    // Find the deps function body
    let deps_start = match content.find("defp deps") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    // Find the opening bracket after defp deps
    let after_deps = &content[deps_start..];
    let bracket_start = match after_deps.find('[') {
        Some(pos) => deps_start + pos,
        None => return Vec::new(),
    };

    // Find matching closing bracket (simple nesting)
    let mut depth = 0;
    let mut bracket_end = bracket_start;
    for (i, ch) in content[bracket_start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    bracket_end = bracket_start + i;
                    break;
                }
            }
            _ => {}
        }
    }

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

        let dependencies = parse_mix_deps(content);

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
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "erlang-dev".into(),
                    "git".into(),
                    "build-base".into(),
                    "openssl".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "mix local.hex --force && mix local.rebar --force".into(),
                    "mix deps.get".into(),
                    "mix compile".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/build/.mix"),
                ]),
                cache_dirs: vec!["deps".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "busybox".into(),
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
                ports: vec![4000],
                health_endpoint: Some("/health".into()),
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(MixExsParser))
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

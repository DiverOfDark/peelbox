//! Elixir language definition

use super::{
    profile::LanguageProfile, Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition,
};
use regex::Regex;

static PROFILE: LanguageProfile = LanguageProfile {
    extensions: &["ex", "exs"],
    excluded_dirs: &["_build", "deps", "cover", ".elixir_ls"],
    runtime_id: crate::RuntimeId::BEAM,
    default_port: Some(4000),
    env_var_patterns: &[(
        r#"System\.get_env\(["']([A-Z_][A-Z0-9_]*)["']"#,
        "System.get_env",
    )],
    port_patterns: &[
        (r#"port:\s*(\d{4,5})"#, "config"),
        (
            r#"Plug\.Cowboy\.http\([^,)]*,\s*port:\s*(\d{4,5})"#,
            "Plug.Cowboy",
        ),
    ],
    health_check_patterns: &[(r#"get\s*"(/health)""#, "Plug.Router")],
    default_health_endpoints: &[("/health", "Default")],
};

pub struct ElixirLanguage;

impl LanguageDefinition for ElixirLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::Elixir
    }

    fn profile(&self) -> &LanguageProfile {
        &PROFILE
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "mix.exs" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("defmodule") && content.contains("def project") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: crate::BuildSystemId::Mix,
                    confidence,
                })
            }
            "mix.lock" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Mix,
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["mix".to_string()]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // mix.exs: elixir: "~> 1.15"
        if let Some(caps) = Regex::new(r#"elixir:\s*"[^"]*(\d+\.\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // .elixir-version file
        if !content.contains("defmodule") {
            let trimmed = content.trim();
            if let Some(caps) = Regex::new(r"^(\d+\.\d+)").ok()?.captures(trimmed) {
                return Some(caps.get(1)?.as_str().to_string());
            }
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

        if let Ok(re) = Regex::new(r#"\{:(\w+),\s*"([^"]+)"\}"#) {
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

        if let Ok(re) = Regex::new(r#"\{:(\w+),\s*path:\s*"([^"]+)"\}"#) {
            for cap in re.captures_iter(manifest_content) {
                if let (Some(name), Some(path_match)) = (cap.get(1), cap.get(2)) {
                    let path_str = path_match.as_str();
                    let is_internal = all_internal_paths
                        .iter()
                        .any(|p| p.to_str().is_some_and(|s| s.contains(path_str)));

                    let dep = Dependency {
                        name: name.as_str().to_string(),
                        version: None,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
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

    fn is_main_file(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        file_path: &std::path::Path,
    ) -> bool {
        if let Some(path_str) = file_path.to_str() {
            if path_str.contains("/lib/") && path_str.ends_with("/application.ex") {
                return true;
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("def start(_type, _args)") || content.contains("use Application") {
                return true;
            }
        }

        false
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("mix phx.server".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, manifest_content: &str) -> Option<String> {
        let app_name = Regex::new(r"app:\s*:(\w+)")
            .ok()?
            .captures(manifest_content)?
            .get(1)?
            .as_str();

        Some(format!("/app/{}/bin/{} start", app_name, app_name))
    }
}

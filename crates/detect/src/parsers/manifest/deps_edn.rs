use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

// Reuse Clojure language and JVM runtime from project_clj.rs
const CLOJURE: LanguageId = LanguageId::new("clojure");
const CLOJURE_TOOLS: BuildSystemId = BuildSystemId::new("clojure-tools");
const JVM: RuntimeId = RuntimeId::new("jvm");

inventory::submit! {
    BuildSystemMeta { slug: "clojure-tools", display_name: "Clojure CLI/tools.build", aliases: &["clojure-tools", "tools-deps"] }
}

pub struct DepsEdnParser;

impl ManifestParser for DepsEdnParser {
    fn filenames(&self) -> &[&str] {
        &["deps.edn"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Basic validation: must look like an EDN map
        let trimmed = content.trim();
        if !trimmed.starts_with('{') {
            return None;
        }

        let java_home = "/usr/lib/jvm/java-17-openjdk";

        // Check if build.clj exists in the same directory (tools.build project)
        let dir = path.parent().unwrap_or(Path::new("."));
        let has_build_clj = dir.join("build.clj").exists();

        // Try to extract a project name from build.clj or use directory name
        let project_name = extract_project_name(dir).unwrap_or_else(|| "app".to_string());

        // Determine build commands and runtime based on whether build.clj exists
        let (build_commands, entrypoint, artifacts) = if has_build_clj {
            // tools.build project: build an uberjar
            let jar_name = format!("{}-standalone.jar", project_name);
            (
                vec![
                    format!(
                        "curl -fsSL https://download.clojure.org/install/linux-install-1.11.1.1435.sh | bash -s -- -p /usr/local"
                    ),
                    "clojure -T:build uber".into(),
                ],
                Some(format!("java -jar /app/{}", jar_name)),
                vec![(format!("target/*-standalone.jar"), format!("/app/{}", jar_name))],
            )
        } else {
            // Plain deps.edn without build.clj — run with clojure directly
            let main_ns = find_main_ns(content);
            if let Some(ns) = main_ns {
                (
                    vec![
                        format!(
                            "curl -fsSL https://download.clojure.org/install/linux-install-1.11.1.1435.sh | bash -s -- -p /usr/local"
                        ),
                        "clojure -P".into(), // Download deps
                    ],
                    Some(format!("clojure -M -m {}", ns)),
                    vec![(".".into(), "/app".into())],
                )
            } else {
                (
                    vec![
                        format!(
                            "curl -fsSL https://download.clojure.org/install/linux-install-1.11.1.1435.sh | bash -s -- -p /usr/local"
                        ),
                        "clojure -P".into(),
                    ],
                    Some("clojure -M -m main".into()),
                    vec![(".".into(), "/app".into())],
                )
            }
        };

        let runtime_env = btree(&[
            ("JAVA_HOME", java_home),
            (
                "PATH",
                &format!(
                    "{}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    java_home
                ),
            ),
        ]);

        Some(Manifest {
            path: path.to_path_buf(),
            language: CLOJURE,
            build_system: CLOJURE_TOOLS,
            runtime: JVM,
            package: Some(Package {
                name: project_name,
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "openjdk-17".into(),
                    "bash".into(),
                    "curl".into(),
                    "ca-certificates".into(),
                ],
                commands: build_commands,
                member_transform: None,
                env: btree(&[
                    ("JAVA_HOME", java_home),
                    (
                        "PATH",
                        &format!(
                            "{}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                            java_home
                        ),
                    ),
                ]),
                cache_dirs: vec![".m2".into(), ".gitlibs".into(), ".cpcache".into()],
                artifacts,
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["openjdk-17-jre".into(), "ca-certificates".into()],
                env: runtime_env,
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

/// Try to extract the project name from build.clj.
/// Looks for patterns like `(def lib 'project-name)` or `(def lib 'ns/project-name)`.
fn extract_project_name(dir: &Path) -> Option<String> {
    let build_clj = dir.join("build.clj");
    let content = std::fs::read_to_string(&build_clj).ok()?;

    // Match: (def lib 'name) or (def lib 'ns/name)
    let re = regex::Regex::new(r#"\(def\s+lib\s+'([\w\-\.\/]+)\)"#).ok()?;
    let caps = re.captures(&content)?;
    let name = caps.get(1)?.as_str();
    // Use the short name (after any namespace separator)
    let short_name = name.rsplit('/').next().unwrap_or(name);
    Some(short_name.to_string())
}

/// Try to find a main namespace reference in deps.edn.
/// Looks for :main-opts ["-m" "some.namespace"] patterns.
fn find_main_ns(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#":main-opts\s+\["-m"\s+"([\w\-\.]+)"\]"#).ok()?;
    let caps = re.captures(content)?;
    Some(caps.get(1)?.as_str().to_string())
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(DepsEdnParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_deps_edn_parser_with_build_clj() {
        let tmp = tempfile::tempdir().unwrap();
        let deps_edn = tmp.path().join("deps.edn");
        let build_clj = tmp.path().join("build.clj");

        let deps_content = r#"{:paths ["src"]
 :deps {}
 :aliases
 {:build {:deps {io.github.clojure/tools.build {:git/tag "v0.9.2" :git/sha "fe6b140"}}
          :ns-default build}}}"#;

        let build_content = r#"(ns build
  (:require [clojure.tools.build.api :as b]))

(def lib 'clojure-example)
(def version "1.0.0")"#;

        std::fs::write(&deps_edn, deps_content).unwrap();
        std::fs::write(&build_clj, build_content).unwrap();

        let parser = DepsEdnParser;
        let manifest = parser.parse(&deps_edn, deps_content).unwrap();

        assert_eq!(manifest.language, LanguageId::new("clojure"));
        assert_eq!(manifest.build_system, BuildSystemId::new("clojure-tools"));
        assert_eq!(manifest.runtime, RuntimeId::new("jvm"));

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "clojure-example");
        assert!(pkg.is_application);

        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("clojure -T:build uber")));
        assert!(manifest
            .runtime_config
            .entrypoint
            .unwrap()
            .contains("java -jar"));
    }

    #[test]
    fn test_deps_edn_parser_without_build_clj() {
        let tmp = tempfile::tempdir().unwrap();
        let deps_edn = tmp.path().join("deps.edn");

        let deps_content = r#"{:paths ["src"]
 :deps {org.clojure/clojure {:mvn/version "1.11.1"}}}"#;

        std::fs::write(&deps_edn, deps_content).unwrap();

        let parser = DepsEdnParser;
        let manifest = parser.parse(&deps_edn, deps_content).unwrap();

        assert_eq!(manifest.language, LanguageId::new("clojure"));
        assert_eq!(manifest.build_system, BuildSystemId::new("clojure-tools"));

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(pkg.is_application);

        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("clojure -P")));
        assert!(manifest
            .runtime_config
            .entrypoint
            .unwrap()
            .contains("clojure -M"));
    }

    #[test]
    fn test_deps_edn_parser_not_edn() {
        let parser = DepsEdnParser;
        let content = "not an edn file";
        let result = parser.parse(Path::new("deps.edn"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_project_name() {
        let tmp = tempfile::tempdir().unwrap();
        let build_clj = tmp.path().join("build.clj");
        std::fs::write(&build_clj, "(def lib 'my-project)").unwrap();
        assert_eq!(
            extract_project_name(tmp.path()),
            Some("my-project".to_string())
        );
    }

    #[test]
    fn test_extract_project_name_with_ns() {
        let tmp = tempfile::tempdir().unwrap();
        let build_clj = tmp.path().join("build.clj");
        std::fs::write(&build_clj, "(def lib 'com.example/my-project)").unwrap();
        assert_eq!(
            extract_project_name(tmp.path()),
            Some("my-project".to_string())
        );
    }

    #[test]
    fn test_find_main_ns() {
        let content = r#"{:aliases {:run {:main-opts ["-m" "my-app.core"]}}}"#;
        assert_eq!(find_main_ns(content), Some("my-app.core".to_string()));
    }

    #[test]
    fn test_find_main_ns_missing() {
        let content = r#"{:paths ["src"] :deps {}}"#;
        assert_eq!(find_main_ns(content), None);
    }
}

use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const CLOJURE: LanguageId = LanguageId::new("clojure");
const LEININGEN: BuildSystemId = BuildSystemId::new("leiningen");
const JVM: RuntimeId = RuntimeId::new("jvm");

inventory::submit! {
    LanguageMeta { slug: "clojure", display_name: "Clojure", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "leiningen", display_name: "Leiningen", aliases: &["lein"] }
}

pub struct ProjectCljParser;

impl ManifestParser for ProjectCljParser {
    fn filenames(&self) -> &[&str] {
        &["project.clj"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("defproject") {
            return None;
        }

        let (project_name, version) = parse_defproject(content)?;
        let main_ns = parse_main_ns(content);
        let dependencies = parse_lein_deps(content);

        let java_home = "/usr/lib/jvm/java-17-openjdk";

        // Build the uberjar filename based on project name
        let jar_name = format!("{}-standalone.jar", project_name);
        let jar_path = format!("target/uberjar/{}", jar_name);

        let entrypoint = Some(format!("java -jar /app/{}", jar_name));

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
            build_system: LEININGEN,
            runtime: JVM,
            package: Some(Package {
                name: project_name.clone(),
                version: Some(version),
                is_application: main_ns.is_some(),
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "openjdk-17".into(),
                    "bash".into(),
                    "curl".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "curl -fsSL https://raw.githubusercontent.com/technomancy/leiningen/stable/bin/lein -o /usr/local/bin/lein && chmod +x /usr/local/bin/lein && lein version".into(),
                    "lein uberjar".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("JAVA_HOME", java_home),
                    ("LEIN_HOME", "/root/.lein"),
                ]),
                cache_dirs: vec![".lein".into(), ".m2".into(), "target".into()],
                artifacts: vec![(jar_path, format!("/app/{}", jar_name))],
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

/// Parse the `(defproject name "version" ...)` form.
/// Returns `(name, version)`.
fn parse_defproject(content: &str) -> Option<(String, String)> {
    let re = regex::Regex::new(r#"\(defproject\s+([\w\-\.\/]+)\s+"([^"]+)""#).ok()?;
    let caps = re.captures(content)?;
    let name = caps.get(1)?.as_str();
    let version = caps.get(2)?.as_str();
    // Use the short name (after any namespace separator)
    let short_name = name.rsplit('/').next().unwrap_or(name);
    Some((short_name.to_string(), version.to_string()))
}

/// Parse `:main` namespace from project.clj.
fn parse_main_ns(content: &str) -> Option<String> {
    let re = regex::Regex::new(r":main\s+([\w\-\.]+)").ok()?;
    let caps = re.captures(content)?;
    Some(caps.get(1)?.as_str().to_string())
}

/// Parse dependencies from `:dependencies [[group/artifact "version"] ...]`.
fn parse_lein_deps(content: &str) -> Vec<Dependency> {
    // Find the :dependencies vector
    let deps_start = match content.find(":dependencies") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let after_deps = &content[deps_start..];
    let bracket_start = match after_deps.find('[') {
        Some(pos) => deps_start + pos,
        None => return Vec::new(),
    };

    // Find matching closing bracket (handle nested brackets for individual dep vectors)
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

    // Match individual dependency vectors: [group/artifact "version"]
    let dep_re = match regex::Regex::new(r#"\[([\w\-\.\/]+)\s+"([^"]+)"\]"#) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    dep_re
        .captures_iter(deps_block)
        .map(|cap| {
            let name = cap[1].to_string();
            let version = Some(cap[2].to_string());
            Dependency {
                name,
                version,
                scope: DepScope::Runtime,
                is_internal: false,
            }
        })
        .collect()
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ProjectCljParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_parse_defproject() {
        let content = r#"(defproject my-app "0.1.0"
  :description "A simple web app"
  :dependencies [[org.clojure/clojure "1.11.1"]]
  :main my-app.core)"#;
        let (name, version) = parse_defproject(content).unwrap();
        assert_eq!(name, "my-app");
        assert_eq!(version, "0.1.0");
    }

    #[test]
    fn test_parse_defproject_with_group() {
        let content = r#"(defproject com.example/my-app "1.0.0"
  :description "An app")"#;
        let (name, version) = parse_defproject(content).unwrap();
        assert_eq!(name, "my-app");
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn test_parse_main_ns() {
        let content = r#"(defproject my-app "0.1.0"
  :main my-app.core)"#;
        assert_eq!(parse_main_ns(content), Some("my-app.core".to_string()));
    }

    #[test]
    fn test_parse_main_ns_missing() {
        let content = r#"(defproject my-app "0.1.0"
  :description "No main")"#;
        assert_eq!(parse_main_ns(content), None);
    }

    #[test]
    fn test_parse_lein_deps() {
        let content = r#"(defproject my-app "0.1.0"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring-core "1.10.0"]
                 [ring/ring-jetty-adapter "1.10.0"]]
  :main my-app.core)"#;
        let deps = parse_lein_deps(content);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].name, "org.clojure/clojure");
        assert_eq!(deps[0].version, Some("1.11.1".to_string()));
        assert_eq!(deps[1].name, "ring/ring-core");
        assert_eq!(deps[1].version, Some("1.10.0".to_string()));
        assert_eq!(deps[2].name, "ring/ring-jetty-adapter");
        assert_eq!(deps[2].version, Some("1.10.0".to_string()));
    }

    #[test]
    fn test_parse_lein_deps_empty() {
        let content = r#"(defproject my-app "0.1.0"
  :description "No deps")"#;
        let deps = parse_lein_deps(content);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_project_clj_parser_basic() {
        let parser = ProjectCljParser;
        let content = r#"(defproject my-app "0.1.0"
  :description "A simple web app"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring-core "1.10.0"]
                 [ring/ring-jetty-adapter "1.10.0"]]
  :main my-app.core
  :profiles {:uberjar {:aot :all}})"#;

        let manifest = parser.parse(Path::new("project.clj"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("clojure"));
        assert_eq!(manifest.build_system, BuildSystemId::new("leiningen"));
        assert_eq!(manifest.runtime, RuntimeId::new("jvm"));

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert_eq!(pkg.version, Some("0.1.0".to_string()));
        assert!(pkg.is_application);

        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.build.commands.len(), 2);
        assert!(manifest.build.commands[0].contains("lein"));
        assert_eq!(manifest.build.commands[1], "lein uberjar");
        assert!(manifest
            .runtime_config
            .entrypoint
            .unwrap()
            .contains("my-app-standalone.jar"));
    }

    #[test]
    fn test_project_clj_parser_not_defproject() {
        let parser = ProjectCljParser;
        let content = r#";; just a comment file
(ns my-app.core)"#;
        assert!(parser.parse(Path::new("project.clj"), content).is_none());
    }

    #[test]
    fn test_project_clj_parser_no_main() {
        let parser = ProjectCljParser;
        let content = r#"(defproject my-lib "1.0.0"
  :description "A library"
  :dependencies [[org.clojure/clojure "1.11.1"]])"#;

        let manifest = parser.parse(Path::new("project.clj"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert!(!pkg.is_application);
    }
}

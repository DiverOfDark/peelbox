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

        // Check for a custom :uberjar-name in :profiles {:uberjar {...}}.
        let custom_uberjar_name = parse_uberjar_name(content);

        // Build the uberjar filename based on project name and version.
        // If :uberjar-name is set in the uberjar profile, use that instead.
        // Check if :target-path has profile-based substitution (%s).
        // When :target-path is "target/%s", `lein uberjar` outputs to target/uberjar/.
        // Otherwise (default), the output goes to target/.
        let has_profile_target = content.contains(":target-path") && content.contains("%s");
        let jar_name = custom_uberjar_name
            .unwrap_or_else(|| format!("{}-{}-standalone.jar", project_name, version));
        let jar_path = if has_profile_target {
            format!("target/uberjar/{}", jar_name)
        } else {
            format!("target/{}", jar_name)
        };

        // Determine the runtime command.
        // When AOT compilation is configured (`:aot :all` in uberjar profile or top-level),
        // the uberjar has a compiled main class and `java -jar` works.
        // Without AOT, use `java -cp <jar> clojure.main -m <ns>` to invoke the namespace.
        let has_aot = has_aot_compilation(content);
        let entrypoint = if !has_aot {
            if let Some(ref ns) = main_ns {
                Some(format!("java -cp /app/{} clojure.main -m {}", jar_name, ns))
            } else {
                Some(format!("java -jar /app/{}", jar_name))
            }
        } else {
            Some(format!("java -jar /app/{}", jar_name))
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
                    "mkdir -p /usr/local/bin && curl -fsSL https://raw.githubusercontent.com/technomancy/leiningen/stable/bin/lein -o /usr/local/bin/lein && chmod +x /usr/local/bin/lein && LEIN_HOME=/app/.lein lein version".into(),
                    "lein uberjar".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("JAVA_HOME", java_home),
                    ("LEIN_HOME", "/app/.lein"),
                    (
                        "PATH",
                        &format!(
                            "{}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                            java_home
                        ),
                    ),
                ]),
                cache_dirs: vec![".lein".into(), ".m2".into()],
                artifacts: vec![(jar_path, format!("/app/{}", jar_name))],
                            setup_commands: vec![],
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
/// Handles metadata annotations like `^:skip-aot` before the namespace name.
fn parse_main_ns(content: &str) -> Option<String> {
    let re = regex::Regex::new(r":main\s+(?:\^:\S+\s+)?([\w\-\.]+)").ok()?;
    let caps = re.captures(content)?;
    Some(caps.get(1)?.as_str().to_string())
}

/// Parse `:uberjar-name` from project.clj (typically inside `:profiles {:uberjar {...}}`).
fn parse_uberjar_name(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#":uberjar-name\s+"([^"]+)""#).ok()?;
    let caps = re.captures(content)?;
    Some(caps.get(1)?.as_str().to_string())
}

/// Detect whether AOT compilation is configured for uberjar builds.
/// Returns true if `:aot :all` or `:aot [...]` appears in the uberjar profile
/// or at the top level of the project.
fn has_aot_compilation(content: &str) -> bool {
    // Check for :aot anywhere in the content — this covers both top-level
    // and profile-nested :aot declarations.
    let re = regex::Regex::new(r":aot\s+(:all|\[)").ok();
    match re {
        Some(re) => re.is_match(content),
        None => false,
    }
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
            .contains("my-app-0.1.0-standalone.jar"));
        // Default: no :target-path with %s, so artifacts go to target/
        assert_eq!(
            manifest.build.artifacts[0].0,
            "target/my-app-0.1.0-standalone.jar"
        );
    }

    #[test]
    fn test_project_clj_parser_with_target_path_profile() {
        let parser = ProjectCljParser;
        let content = r#"(defproject my-app "0.1.0"
  :description "A web app with target-path"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring-core "1.10.0"]]
  :main my-app.core
  :target-path "target/%s"
  :profiles {:uberjar {:aot :all}})"#;

        let manifest = parser.parse(Path::new("project.clj"), content).unwrap();
        // When :target-path has %s, artifacts go to target/uberjar/
        assert_eq!(
            manifest.build.artifacts[0].0,
            "target/uberjar/my-app-0.1.0-standalone.jar"
        );
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

    #[test]
    fn test_parse_main_ns_with_skip_aot() {
        let content = r#"(defproject my-app "0.1.0"
  :main ^:skip-aot my-app.core)"#;
        assert_eq!(parse_main_ns(content), Some("my-app.core".to_string()));
    }

    #[test]
    fn test_parse_uberjar_name() {
        let content = r#"(defproject my-app "0.1.0"
  :profiles {:uberjar {:uberjar-name "my-app.jar"}})"#;
        assert_eq!(parse_uberjar_name(content), Some("my-app.jar".to_string()));
    }

    #[test]
    fn test_parse_uberjar_name_missing() {
        let content = r#"(defproject my-app "0.1.0"
  :profiles {:uberjar {:aot :all}})"#;
        assert_eq!(parse_uberjar_name(content), None);
    }

    #[test]
    fn test_has_aot_compilation_all() {
        let content = r#"(defproject my-app "0.1.0"
  :profiles {:uberjar {:aot :all}})"#;
        assert!(has_aot_compilation(content));
    }

    #[test]
    fn test_has_aot_compilation_vector() {
        let content = r#"(defproject my-app "0.1.0"
  :profiles {:uberjar {:aot [my-app.core]}})"#;
        assert!(has_aot_compilation(content));
    }

    #[test]
    fn test_has_aot_compilation_missing() {
        let content = r#"(defproject my-app "0.1.0"
  :main my-app.core)"#;
        assert!(!has_aot_compilation(content));
    }

    #[test]
    fn test_no_aot_uses_clojure_main() {
        let parser = ProjectCljParser;
        let content = r#"(defproject hello-ring "0.1.0-SNAPSHOT"
  :dependencies [[org.clojure/clojure "1.10.3"]
                 [ring/ring-core "1.9.5"]
                 [ring/ring-jetty-adapter "1.9.5"]]
  :main hello-ring.core)"#;

        let manifest = parser.parse(Path::new("project.clj"), content).unwrap();
        assert_eq!(
            manifest.runtime_config.entrypoint.unwrap(),
            "java -cp /app/hello-ring-0.1.0-SNAPSHOT-standalone.jar clojure.main -m hello-ring.core"
        );
    }

    #[test]
    fn test_aot_uses_java_jar() {
        let parser = ProjectCljParser;
        let content = r#"(defproject my-app "0.1.0"
  :dependencies [[org.clojure/clojure "1.11.1"]]
  :main my-app.core
  :profiles {:uberjar {:aot :all}})"#;

        let manifest = parser.parse(Path::new("project.clj"), content).unwrap();
        assert_eq!(
            manifest.runtime_config.entrypoint.unwrap(),
            "java -jar /app/my-app-0.1.0-standalone.jar"
        );
    }

    #[test]
    fn test_custom_uberjar_name_used_in_artifacts() {
        let parser = ProjectCljParser;
        let content = r#"(defproject my-app "0.1.0-SNAPSHOT"
  :dependencies [[org.clojure/clojure "1.11.1"]]
  :main ^:skip-aot my-app.core
  :target-path "target/%s/"
  :profiles {:uberjar {:aot :all
                       :uberjar-name "my-app.jar"}})"#;

        let manifest = parser.parse(Path::new("project.clj"), content).unwrap();
        assert_eq!(manifest.build.artifacts[0].0, "target/uberjar/my-app.jar");
        assert_eq!(manifest.build.artifacts[0].1, "/app/my-app.jar");
        assert_eq!(
            manifest.runtime_config.entrypoint.unwrap(),
            "java -jar /app/my-app.jar"
        );
    }
}

use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const SCALA: LanguageId = LanguageId::new("scala");
const SBT: BuildSystemId = BuildSystemId::new("sbt");
const JVM: RuntimeId = RuntimeId::new("jvm");

inventory::submit! {
    LanguageMeta { slug: "scala", display_name: "Scala", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "sbt", display_name: "sbt", aliases: &["sbt"] }
}

pub struct BuildSbtParser;

impl ManifestParser for BuildSbtParser {
    fn filenames(&self) -> &[&str] {
        &["build.sbt"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Basic validation: must contain some sbt-like content
        if !content.contains(":=") && !content.contains("++=") && !content.contains("+=") {
            return None;
        }

        let scala_version = extract_scala_version(content);
        let project_name = extract_project_name(content);
        let project_version = extract_project_version(content);
        let dependencies = parse_sbt_deps(content);

        let java_version = crate::version::java::detect_java_version(content);
        let java_pkg = java_version
            .as_ref()
            .map(|v| format!("openjdk-{}", v))
            .unwrap_or_else(|| "openjdk".into());

        let java_home = format!(
            "/usr/lib/jvm/java-{}-openjdk",
            java_version.as_deref().unwrap_or("21")
        );

        // Detect if this is an application (has mainClass setting or a plugin like sbt-assembly)
        let has_main_class =
            content.contains("mainClass") || content.contains("Compile / mainClass");
        let has_assembly = content.contains("assembly");
        let has_native_packager = content.contains("JavaAppPackaging")
            || content.contains("sbt-native-packager")
            || content.contains("DockerPlugin");
        let is_application = has_main_class || has_assembly || has_native_packager;

        let jar_name = match (&project_name, &project_version, &scala_version) {
            (Some(name), Some(ver), Some(sv)) => {
                let scala_major = scala_binary_version(sv);
                format!("{}_{}_{}", name, scala_major, ver)
            }
            (Some(name), Some(ver), None) => format!("{}-{}", name, ver),
            (Some(name), None, _) => name.clone(),
            _ => "app".to_string(),
        };

        let assembly_jar = format!("{}-assembly.jar", jar_name);

        let entrypoint = if is_application {
            Some(format!("java -jar /app/{}", assembly_jar))
        } else {
            None
        };

        let build_commands = if has_assembly {
            vec!["sbt assembly".into()]
        } else {
            vec!["sbt compile".into()]
        };

        let artifacts = if has_assembly {
            vec![(
                format!(
                    "target/scala-*/{}*-assembly.jar",
                    project_name.as_deref().unwrap_or("app")
                ),
                "/app/app.jar".into(),
            )]
        } else {
            vec![("target/scala-*/*.jar".into(), "/app/app.jar".into())]
        };

        let member_transform = Some(MemberBuildTransform {
            member_commands: vec!["sbt {module}/compile".into()],
            member_artifacts: Some(vec![(
                "{module}/target/scala-*/*.jar".into(),
                "/app/app.jar".into(),
            )]),
        });

        let runtime_env = btree(&[
            ("JAVA_HOME", &java_home),
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
            language: SCALA,
            build_system: SBT,
            runtime: JVM,
            package: Some(Package {
                name: project_name.unwrap_or_default(),
                version: project_version,
                is_application,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![java_pkg.clone(), "sbt".into(), "ca-certificates".into()],
                commands: build_commands,
                member_transform,
                env: btree(&[
                    ("JAVA_HOME", &java_home),
                    (
                        "SBT_OPTS",
                        "-Dsbt.global.base=/root/.sbt -Dsbt.ivy.home=/root/.ivy2",
                    ),
                ]),
                cache_dirs: vec![".ivy2".into(), ".sbt".into(), "target".into()],
                artifacts,
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    java_version
                        .as_ref()
                        .map(|v| format!("openjdk-{}-jre", v))
                        .unwrap_or_else(|| "openjdk".into()),
                    "ca-certificates".into(),
                ],
                env: runtime_env,
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Extract Scala version from `scalaVersion := "X.Y.Z"`.
fn extract_scala_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"scalaVersion\s*:=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Extract project name from `name := "my-project"`.
fn extract_project_name(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*name\s*:=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Extract project version from `version := "X.Y.Z"`.
fn extract_project_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*version\s*:=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Compute the Scala binary version (e.g., "3.3.1" -> "3", "2.13.12" -> "2.13").
fn scala_binary_version(full_version: &str) -> String {
    let parts: Vec<&str> = full_version.splitn(3, '.').collect();
    match parts.first() {
        Some(&"3") => "3".to_string(),
        Some(major) if parts.len() >= 2 => format!("{}.{}", major, parts[1]),
        _ => full_version.to_string(),
    }
}

/// Parse sbt-style library dependencies.
///
/// Handles formats like:
/// - `"org" %% "artifact" % "version"`
/// - `"org" % "artifact" % "version"`
/// - `"org" %% "artifact" % "version" % Test`
fn parse_sbt_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();

    // Match: "groupId" %% "artifactId" % "version" [% scope]
    // Also: "groupId" % "artifactId" % "version" [% scope]
    let re = regex::Regex::new(r#""([^"]+)"\s*%%?\s*"([^"]+)"\s*%\s*"([^"]+)"(?:\s*%\s*(\w+))?"#)
        .unwrap();

    for cap in re.captures_iter(content) {
        let group = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let artifact = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let version = cap.get(3).map(|m| m.as_str());
        let scope_str = cap.get(4).map(|m| m.as_str());

        let scope = match scope_str {
            Some("Test") | Some("test") => DepScope::Dev,
            Some("Provided") | Some("provided") => DepScope::Build,
            _ => DepScope::Runtime,
        };

        deps.push(Dependency {
            name: format!("{}:{}", group, artifact),
            version: version.map(String::from),
            scope,
            is_internal: false,
        });
    }

    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(BuildSbtParser))
}

// ── Build System Profile ────────────────────────────────────────────────────

fn sbt_subdirectory_command(cmd: &str, subdir: &str) -> String {
    if cmd.starts_with("sbt ") {
        format!("cd {} && {}", subdir, cmd)
    } else {
        format!("cd {} && {}", subdir, cmd)
    }
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        transform_subdirectory_command: sbt_subdirectory_command,
        ..BuildSystemConfig::new(SBT)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_build_sbt_basic() {
        let content = r#"
name := "my-app"
version := "0.1.0"
scalaVersion := "3.3.1"

libraryDependencies ++= Seq(
  "com.typesafe.akka" %% "akka-http" % "10.5.3",
  "com.typesafe.akka" %% "akka-stream" % "2.8.5"
)
"#;
        let manifest = BuildSbtParser
            .parse(Path::new("build.sbt"), content)
            .unwrap();
        assert_eq!(manifest.language, LanguageId::new("scala"));
        assert_eq!(manifest.build_system, BuildSystemId::new("sbt"));
        assert_eq!(manifest.runtime, RuntimeId::new("jvm"));

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert_eq!(pkg.version, Some("0.1.0".into()));
        assert!(!pkg.is_application);

        assert_eq!(manifest.dependencies.len(), 2);
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "com.typesafe.akka:akka-http"));
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "com.typesafe.akka:akka-stream"));
    }

    #[test]
    fn test_build_sbt_with_assembly() {
        let content = r#"
name := "my-service"
version := "1.0.0"
scalaVersion := "2.13.12"
assembly / mainClass := Some("com.example.Main")

libraryDependencies += "com.typesafe.akka" %% "akka-http" % "10.5.3"
"#;
        let manifest = BuildSbtParser
            .parse(Path::new("build.sbt"), content)
            .unwrap();
        let pkg = manifest.package.unwrap();
        assert!(pkg.is_application);
        assert_eq!(manifest.build.commands, vec!["sbt assembly"]);
        assert!(manifest.runtime_config.entrypoint.is_some());
    }

    #[test]
    fn test_build_sbt_with_test_deps() {
        let content = r#"
name := "my-app"
scalaVersion := "3.3.1"

libraryDependencies ++= Seq(
  "com.typesafe.akka" %% "akka-http" % "10.5.3",
  "org.scalatest" %% "scalatest" % "3.2.17" % Test
)
"#;
        let manifest = BuildSbtParser
            .parse(Path::new("build.sbt"), content)
            .unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        let test_dep = manifest
            .dependencies
            .iter()
            .find(|d| d.name.contains("scalatest"))
            .unwrap();
        assert_eq!(test_dep.scope, DepScope::Dev);
    }

    #[test]
    fn test_scala_binary_version() {
        assert_eq!(scala_binary_version("3.3.1"), "3");
        assert_eq!(scala_binary_version("2.13.12"), "2.13");
        assert_eq!(scala_binary_version("2.12.18"), "2.12");
    }

    #[test]
    fn test_extract_scala_version() {
        assert_eq!(
            extract_scala_version(r#"scalaVersion := "3.3.1""#),
            Some("3.3.1".into())
        );
        assert_eq!(
            extract_scala_version(r#"scalaVersion := "2.13.12""#),
            Some("2.13.12".into())
        );
        assert_eq!(extract_scala_version("no version here"), None);
    }

    #[test]
    fn test_non_sbt_content_returns_none() {
        let content = "this is not a build.sbt file at all";
        assert!(BuildSbtParser
            .parse(Path::new("build.sbt"), content)
            .is_none());
    }
}

use crate::helpers::btree;
use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

pub struct BuildGradleParser;

impl ManifestParser for BuildGradleParser {
    fn filenames(&self) -> &[&str] {
        &["build.gradle", "build.gradle.kts"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("plugins") && !content.contains("dependencies") {
            return None;
        }

        let java_version = crate::version::java::detect_java_version(content);
        let java_pkg = java_version
            .as_ref()
            .map(|v| format!("openjdk-{}", v))
            .unwrap_or_else(|| "openjdk".into());

        let dependencies = parse_gradle_deps(content);

        // Try to extract version from build.gradle for artifact naming
        let _project_version = regex::Regex::new(r#"version\s*=?\s*['"]([^'"]+)['"]"#)
            .ok()
            .and_then(|re| {
                re.captures(content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            });

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Java,
            build_system: BuildSystemId::Gradle,
            runtime: RuntimeId::JVM,
            package: None, // Gradle project names come from settings.gradle
            workspace: None, // Workspace comes from SettingsGradleParser
            dependencies,
            build: BuildSpec {
                packages: vec![java_pkg.clone(), "gradle".into(), "ca-certificates".into()],
                commands: vec![
                    "gradle assemble -x test --no-daemon --console=plain".into(),
                ],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        "gradle :{module}:assemble -x test --no-daemon --console=plain".into(),
                    ],
                    member_artifacts: Some(vec![(
                        "{module}/build/libs/*.jar".into(),
                        "/app/app.jar".into(),
                    )]),
                }),
                env: btree(&[
                    (
                        "JAVA_HOME",
                        &format!(
                            "/usr/lib/jvm/java-{}-openjdk",
                            java_version.as_deref().unwrap_or("21")
                        ),
                    ),
                    ("GRADLE_USER_HOME", "/root/.gradle"),
                    ("GRADLE_OPTS", "-Dorg.gradle.native=false"),
                ]),
                cache_dirs: vec![".gradle".into(), "build".into()],
                artifacts: vec![("build/libs/*.jar".into(), "/app/app.jar".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    java_version
                        .as_ref()
                        .map(|v| format!("openjdk-{}-jre", v))
                        .unwrap_or_else(|| "openjdk".into()),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: Some("java -jar /app/app.jar".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

fn parse_gradle_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let dep_re =
        regex::Regex::new(r#"(?:implementation|api|compile|runtimeOnly|testImplementation)\s*[\("]([^")\s]+)"#)
            .unwrap();
    for cap in dep_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            let dep_str = m.as_str();
            let parts: Vec<&str> = dep_str.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let scope = if cap
                    .get(0)
                    .map(|m| m.as_str().contains("test"))
                    .unwrap_or(false)
                {
                    DepScope::Dev
                } else {
                    DepScope::Runtime
                };
                deps.push(Dependency {
                    name: format!("{}:{}", parts[0], parts[1]),
                    version: parts.get(2).map(|s| s.to_string()),
                    scope,
                    is_internal: false,
                });
            }
        }
    }
    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(BuildGradleParser))
}

use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const JAVA: LanguageId = LanguageId::new("java");
const GRADLE: BuildSystemId = BuildSystemId::new("gradle");
const JVM: RuntimeId = RuntimeId::new("jvm");

inventory::submit! {
    LanguageMeta { slug: "kotlin", display_name: "Kotlin", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "gradle", display_name: "Gradle", aliases: &["gradle"] }
}

pub struct BuildGradleParser;

impl ManifestParser for BuildGradleParser {
    fn filenames(&self) -> &[&str] {
        &["build.gradle", "build.gradle.kts"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("plugins") && !content.contains("dependencies") {
            return None;
        }

        let java_version = crate::version::java::detect_java_version(content).or_else(|| {
            // When no explicit Java version is specified, derive a compatible
            // version from the Gradle wrapper (if present). Newer JDKs are not
            // supported by older Gradle versions.
            path.parent()
                .and_then(max_jdk_for_gradle_wrapper)
                .map(|v: u32| v.to_string())
        });
        let java_pkg = java_version
            .as_ref()
            .map(|v| format!("openjdk-{}", v))
            .unwrap_or_else(|| "openjdk".into());

        let dependencies = parse_gradle_deps(content);

        // Extract version from build.gradle (e.g., `version = '1.0.0'` or `version = "1.0.0"`)
        let gradle_version = regex::Regex::new(r#"(?m)^version\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

        // Extract project name from build.gradle content (not from directory — that's unreliable)
        let project_name = regex::Regex::new(r#"(?m)archivesBaseName\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .or_else(|| {
                regex::Regex::new(r#"(?m)rootProject\.name\s*=\s*["']([^"']+)["']"#)
                    .ok()
                    .and_then(|re| re.captures(content))
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            });

        // Detect if this is an application project (has spring-boot or application plugin)
        let has_spring_boot =
            content.contains("spring-boot") || content.contains("org.springframework.boot");
        let has_application_plugin = content.contains("'application'")
            || content.contains("\"application\"")
            || content.contains("id(\"application\")");
        let is_application = has_spring_boot || has_application_plugin;

        let java_home = format!(
            "/usr/lib/jvm/java-{}-openjdk",
            java_version.as_deref().unwrap_or("21")
        );

        // Check if the project has a Gradle wrapper (gradlew) for version-specific builds
        let has_gradlew = path
            .parent()
            .map(|dir| dir.join("gradlew").exists())
            .unwrap_or(false);
        let gradle_cmd = if has_gradlew { "./gradlew" } else { "gradle" };

        let entrypoint = if is_application {
            match (&project_name, &gradle_version) {
                (Some(name), Some(ver)) => Some(format!("java -jar /app/{}-{}.jar", name, ver)),
                _ => Some("java -jar /app/app.jar".into()),
            }
        } else {
            None
        };

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
            language: JAVA,
            build_system: GRADLE,
            runtime: JVM,
            package: gradle_version.as_ref().map(|v| Package {
                name: String::new(), // Will be filled from settings.gradle merge
                version: Some(v.clone()),
                is_application,
            }),
            workspace: None, // Workspace comes from SettingsGradleParser
            dependencies,
            build: BuildSpec {
                packages: {
                    let mut pkgs = vec![java_pkg.clone()];
                    if has_gradlew {
                        pkgs.push("bash".into());
                    } else {
                        pkgs.push("gradle-8".into());
                    }
                    pkgs.push("ca-certificates".into());
                    pkgs
                },
                commands: vec![format!(
                    "{} assemble -x test --no-daemon --console=plain",
                    gradle_cmd
                )],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![format!(
                        "{} :{{module}}:assemble -x test --no-daemon --console=plain",
                        gradle_cmd
                    )],
                    member_artifacts: Some(vec![(
                        "{module}/build/libs/*.jar".into(),
                        "/app/app.jar".into(),
                    )]),
                }),
                env: btree(&[
                    ("JAVA_HOME", &java_home),
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
                env: runtime_env,
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Parse the Gradle version from `gradle/wrapper/gradle-wrapper.properties`
/// and return the maximum JDK version that Gradle version supports.
fn max_jdk_for_gradle_wrapper(project_dir: &Path) -> Option<u32> {
    let props_path = project_dir.join("gradle/wrapper/gradle-wrapper.properties");
    let content = std::fs::read_to_string(props_path).ok()?;
    let re = regex::Regex::new(r"gradle-(\d+)\.(\d+)").ok()?;
    let caps = re.captures(&content)?;
    let major: u32 = caps.get(1)?.as_str().parse().ok()?;
    let minor: u32 = caps.get(2)?.as_str().parse().ok()?;

    let max_jdk = match (major, minor) {
        (5, _) => 12,
        (6, _) => 15,
        (7, 0..=2) => 16,
        (7, 3..=4) => 17,
        (7, 5) => 18,
        (7, _) => 19,
        (8, 0..=3) => 20,
        (8, 4..=5) => 21,
        (8, 6..=8) => 22,
        (8, 9..=11) => 23,
        (8, _) => 24,
        _ => return None,
    };
    Some(max_jdk)
}

fn parse_gradle_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let dep_re = regex::Regex::new(
        r#"(?:implementation|api|compile|runtimeOnly|testImplementation)\s*\(?["']([^"')\s]+)"#,
    )
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

// ── Build System Profile ────────────────────────────────────────────────────

fn gradle_resolve_artifacts(
    artifacts: &mut [peelbox_core::output::schema::CopySpec],
    package: Option<&Package>,
) {
    if let Some(pkg) = package {
        if let Some(version) = &pkg.version {
            let specific_jar = format!("{}-{}.jar", pkg.name, version);
            for artifact in artifacts.iter_mut() {
                if artifact.from.contains("*.jar") {
                    artifact.from = artifact.from.replace("*.jar", &specific_jar);
                }
            }
        }
    }
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        use_package_name_for_root: false,
        resolve_artifacts: gradle_resolve_artifacts,
        ..BuildSystemConfig::new(GRADLE)
    })
}

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
        if !content.contains("plugins")
            && !content.contains("dependencies")
            && !content.contains("apply plugin")
        {
            return None;
        }

        let java_version = super::pom_xml::detect_java_version(content).or_else(|| {
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

        // Extract Main-Class from jar manifest attributes (both Groovy and Kotlin DSL)
        // Groovy: attributes('Main-Class': 'com.example.Main')
        // Kotlin DSL: attributes["Main-Class"] = "com.example.Main"
        //          or attributes("Main-Class" to "com.example.Main")
        let jar_main_class = regex::Regex::new(
            r#"(?s)(?:attributes\s*\(.*?['"]Main-Class['"]\s*(?::|to)\s*['"]([^'"]+)['"]|attributes\s*\[\s*"Main-Class"\s*\]\s*=\s*"([^"]+)")"#
        )
        .ok()
        .and_then(|re| {
            re.captures(content).map(|c| {
                c.get(1)
                    .or_else(|| c.get(2))
                    .map(|m| m.as_str().to_string())
            })
        })
        .flatten();

        // Detect if this is an application project (has spring-boot or application plugin)
        let has_spring_boot =
            content.contains("spring-boot") || content.contains("org.springframework.boot");
        let has_shadow_plugin = content.contains("shadow")
            || content.contains("com.github.johnrengelman.shadow")
            || content.contains("com.gradleup.shadow");
        let has_application_plugin = content.contains("'application'")
            || content.contains("\"application\"")
            || content.contains("id(\"application\")")
            || is_bare_application_plugin(content);
        let has_jar_main_class = jar_main_class.is_some();
        let is_application =
            has_spring_boot || has_shadow_plugin || has_application_plugin || has_jar_main_class;

        // Application plugin without a fat-jar producer (Spring Boot / Shadow)
        // needs installDist instead of assemble, since assemble only creates a thin jar.
        // Only use installDist when mainClass is explicitly configured in the application
        // block — without it, the generated start script has no class to invoke.
        let has_main_class = content.contains("mainClass") || content.contains("mainClassName");
        let use_install_dist =
            has_application_plugin && has_main_class && !has_spring_boot && !has_shadow_plugin;

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

        let entrypoint = if use_install_dist {
            // installDist produces scripts in build/install/{name}/bin/{name}
            // After copying to /app, the start script is at /app/bin/{name}
            Some("/app/bin/app".into())
        } else if is_application {
            // is_application covers: spring-boot, shadow, application plugin, or jar Main-Class
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

        // Detect broken Kotlin DSL syntax in .kts files: Groovy-style patterns
        // that are invalid in Kotlin DSL. Common when a .gradle file is renamed to .kts.
        let is_kts = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".kts"))
            .unwrap_or(false);
        // `"key" = "value"` inside attributes() should be `"key" to "value"`
        let needs_kts_attr_fix = is_kts
            && regex::Regex::new(r#"(?s)attributes\s*\(.*?"[^"]*"\s*=\s*"[^"]*".*?\)"#)
                .ok()
                .map(|r| r.is_match(content))
                .unwrap_or(false);
        // `jar {` at top level (Groovy) should be `tasks.jar {` in Kotlin DSL
        let needs_kts_jar_fix = is_kts
            && regex::Regex::new(r#"(?m)^jar\s*\{"#)
                .ok()
                .map(|r| r.is_match(content))
                .unwrap_or(false);

        // Build commands and artifacts differ for installDist vs assemble
        let (build_command, member_transform, artifacts) = if use_install_dist {
            (
                format!(
                    "{} installDist -x test --no-daemon --console=plain",
                    gradle_cmd
                ),
                Some(MemberBuildTransform {
                    member_commands: vec![format!(
                        "{} :{{module}}:installDist -x test --no-daemon --console=plain",
                        gradle_cmd
                    )],
                    member_artifacts: Some(vec![(
                        "{module}/build/install/{module}/.".into(),
                        "/app".into(),
                    )]),
                }),
                vec![("build/install/.".into(), "/app".into())],
            )
        } else {
            (
                format!(
                    "{} assemble -x test --no-daemon --console=plain",
                    gradle_cmd
                ),
                Some(MemberBuildTransform {
                    member_commands: vec![format!(
                        "{} :{{module}}:assemble -x test --no-daemon --console=plain",
                        gradle_cmd
                    )],
                    member_artifacts: Some(vec![(
                        "{module}/build/libs/*.jar".into(),
                        "/app/app.jar".into(),
                    )]),
                }),
                vec![("build/libs/*.jar".into(), "/app/app.jar".into())],
            )
        };

        // When a .kts file has Groovy-style syntax, prepend sed commands to fix
        // it to valid Kotlin DSL before running the Gradle build.
        let mut build_commands = Vec::new();
        if needs_kts_attr_fix || needs_kts_jar_fix {
            let kts_filename = path.file_name().unwrap().to_string_lossy();
            if needs_kts_attr_fix {
                build_commands.push(format!(
                    "sed -i 's/\"\\([^\"]*\\)\"[[:space:]]*=[[:space:]]*\"\\([^\"]*\\)\"/\"\\1\" to \"\\2\"/g' {}",
                    kts_filename
                ));
            }
            if needs_kts_jar_fix {
                build_commands.push(format!("sed -i 's/^jar {{/tasks.jar {{/' {}", kts_filename));
            }
        }
        build_commands.push(build_command);

        // Construct Docker Hub build image: gradle:{major}-jdk{jdk}
        let build_image = gradle_build_image(path.parent(), java_version.as_deref());

        // installDist produces shell scripts (#!/bin/sh) that need a POSIX shell
        // and basic utilities (ls, uname, xargs, sed, etc.) at runtime.
        // busybox provides /bin/sh and all required utilities.
        let mut runtime_packages = vec![
            java_version
                .as_ref()
                .map(|v| format!("openjdk-{}-jre", v))
                .unwrap_or_else(|| "openjdk".into()),
            "ca-certificates".into(),
        ];
        if use_install_dist {
            runtime_packages.push("busybox".into());
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: JAVA,
            build_system: GRADLE,
            runtime: JVM,
            package: if gradle_version.is_some() || is_application {
                Some(Package {
                    name: String::new(), // Will be filled from settings.gradle merge
                    version: gradle_version.clone(),
                    is_application,
                })
            } else {
                None
            },
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
                commands: build_commands,
                member_transform,
                env: btree(&[
                    ("JAVA_HOME", &java_home),
                    ("GRADLE_USER_HOME", "/root/.gradle"),
                    ("GRADLE_OPTS", "-Dorg.gradle.native=false"),
                ]),
                cache_dirs: vec![".gradle".into(), "build".into()],
                artifacts,
                build_image: Some(build_image),
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: runtime_env,
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Detect the bare `application` keyword inside a `plugins { ... }` block
/// in Kotlin DSL. This is the shorthand for `id("application")`.
/// We avoid false positives by only matching inside the plugins block.
fn is_bare_application_plugin(content: &str) -> bool {
    // Find the plugins block and check for bare `application` keyword (Kotlin DSL shorthand)
    if let Some(plugins_start) = content.find("plugins") {
        let rest = &content[plugins_start..];
        if let Some(brace_start) = rest.find('{') {
            let inner = &rest[brace_start + 1..];
            if let Some(brace_end) = inner.find('}') {
                let block = &inner[..brace_end];
                // Check for `application` as a standalone token on a line
                return block.lines().any(|line| {
                    let trimmed = line.trim();
                    // Match bare `application` or `application` followed by a comment
                    trimmed == "application"
                        || trimmed.starts_with("application //")
                        || trimmed.starts_with("application /*")
                });
            }
        }
    }
    false
}

/// Parse Gradle major version from `gradle/wrapper/gradle-wrapper.properties`.
fn gradle_wrapper_major(project_dir: &Path) -> Option<u32> {
    let props_path = project_dir.join("gradle/wrapper/gradle-wrapper.properties");
    let content = std::fs::read_to_string(props_path).ok()?;
    let re = regex::Regex::new(r"gradle-(\d+)\.(\d+)").ok()?;
    let caps = re.captures(&content)?;
    caps.get(1)?.as_str().parse().ok()
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
        (8, 0..=2) => 19,
        (8, 3) => 20,
        (8, 4..=5) => 21,
        (8, 6..=8) => 22,
        (8, 9..=11) => 23,
        (8, _) => 24,
        _ => return None,
    };
    Some(max_jdk)
}

/// Return the Docker Hub build image for a Gradle project.
/// Format: `docker.io/library/gradle:{gradle_major}-jdk{jdk}`
/// Falls back to `gradle:latest-jdk{jdk}` when Gradle version is unknown,
/// and defaults JDK to 21 (current LTS) when unspecified.
fn gradle_build_image(project_dir: Option<&Path>, jdk_version: Option<&str>) -> String {
    let jdk = jdk_version.unwrap_or("21");
    let gradle_major = project_dir.and_then(gradle_wrapper_major);
    match gradle_major {
        Some(major) => format!("docker.io/library/gradle:{}-jdk{}", major, jdk),
        None => format!("docker.io/library/gradle:jdk{}", jdk),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;
    use std::io::Write;

    /// A .kts file with Groovy-style `"key" = "value"` in `attributes()` should
    /// get a sed pre-build command to fix the syntax to Kotlin DSL (`"key" to "value"`).
    #[test]
    fn test_kts_broken_attributes_gets_sed_fix() {
        let content = r#"
plugins {
    id("application")
}
repositories { mavenCentral() }
dependencies {
    implementation("com.google.guava:guava:31.1-jre")
}
jar {
    manifest {
        attributes(
            "Main-Class" = "gradle.App"
        )
    }
}
"#;
        let dir = tempfile::tempdir().unwrap();
        let kts_path = dir.path().join("build.gradle.kts");
        std::fs::write(&kts_path, content).unwrap();
        // Create a fake gradlew so the parser uses ./gradlew
        let gradlew_path = dir.path().join("gradlew");
        let mut f = std::fs::File::create(&gradlew_path).unwrap();
        f.write_all(b"#!/bin/sh\n").unwrap();

        let parser = BuildGradleParser;
        let manifest = parser.parse(&kts_path, content).unwrap();

        assert_eq!(manifest.build.commands.len(), 3);
        assert!(
            manifest.build.commands[0].starts_with("sed -i"),
            "First command should be a sed fix for attributes: {}",
            manifest.build.commands[0]
        );
        assert!(
            manifest.build.commands[0].contains("build.gradle.kts"),
            "sed should target build.gradle.kts"
        );
        assert!(
            manifest.build.commands[1].contains("tasks.jar"),
            "Second command should fix jar -> tasks.jar: {}",
            manifest.build.commands[1]
        );
        assert!(
            manifest.build.commands[2].contains("assemble"),
            "Third command should be gradlew assemble"
        );
    }

    /// A non-kts (Groovy) build.gradle should NOT get a sed fix, even with the
    /// same content pattern.
    #[test]
    fn test_groovy_gradle_no_sed_fix() {
        let content = r#"
plugins {
    id 'application'
}
repositories { mavenCentral() }
dependencies {
    implementation 'com.google.guava:guava:31.1-jre'
}
jar {
    manifest {
        attributes(
            'Main-Class': 'gradle.App'
        )
    }
}
"#;
        let dir = tempfile::tempdir().unwrap();
        let gradle_path = dir.path().join("build.gradle");
        std::fs::write(&gradle_path, content).unwrap();
        let gradlew_path = dir.path().join("gradlew");
        let mut f = std::fs::File::create(&gradlew_path).unwrap();
        f.write_all(b"#!/bin/sh\n").unwrap();

        let parser = BuildGradleParser;
        let manifest = parser.parse(&gradle_path, content).unwrap();

        assert_eq!(manifest.build.commands.len(), 1);
        assert!(
            manifest.build.commands[0].contains("assemble"),
            "Should have only the assemble command"
        );
    }

    /// A valid .kts file without any broken patterns should NOT get a sed fix.
    #[test]
    fn test_valid_kts_no_sed_fix() {
        let content = r#"
plugins {
    id("application")
}
repositories { mavenCentral() }
dependencies {
    implementation("com.google.guava:guava:31.1-jre")
}
tasks.jar {
    manifest {
        attributes(
            "Main-Class" to "gradle.App"
        )
    }
}
"#;
        let dir = tempfile::tempdir().unwrap();
        let kts_path = dir.path().join("build.gradle.kts");
        std::fs::write(&kts_path, content).unwrap();
        let gradlew_path = dir.path().join("gradlew");
        let mut f = std::fs::File::create(&gradlew_path).unwrap();
        f.write_all(b"#!/bin/sh\n").unwrap();

        let parser = BuildGradleParser;
        let manifest = parser.parse(&kts_path, content).unwrap();

        assert_eq!(manifest.build.commands.len(), 1);
        assert!(
            manifest.build.commands[0].contains("assemble"),
            "Should have only the assemble command"
        );
    }
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
        // For installDist artifacts, resolve the project name in the install directory path.
        // installDist creates build/install/{projectName}/ — replace the generic path
        // with the resolved project name.
        if !pkg.name.is_empty() {
            for artifact in artifacts.iter_mut() {
                if artifact.from == "build/install/." {
                    artifact.from = format!("build/install/{}/.", pkg.name);
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

// ── Gradle Java version parsing ─────────────────────────────────────────

/// Parse Java version from build.gradle or build.gradle.kts content.
/// Looks for `sourceCompatibility`, `targetCompatibility`, `languageVersion`.
/// Returns raw version number (e.g., "17").
pub(crate) fn parse_gradle_version(content: &str) -> Option<String> {
    // sourceCompatibility = JavaVersion.VERSION_17 or "17" or JavaVersion.VERSION_1_8
    if let Some(caps) =
        regex::Regex::new(r#"sourceCompatibility\s*=\s*(?:JavaVersion\.VERSION_)?["']?([\d._]+)"#)
            .ok()
            .and_then(|re| re.captures(content))
    {
        let raw = caps.get(1).map(|m| m.as_str())?;
        // Handle VERSION_1_8 style (underscores become dots)
        let cleaned = raw.replace('_', ".");
        return Some(cleaned);
    }

    // languageVersion.set(JavaLanguageVersion.of(21)) or languageVersion = JavaLanguageVersion.of(21)
    if let Some(caps) = regex::Regex::new(
        r"languageVersion(?:\.set)?(?:\s*=\s*|\s+|\()JavaLanguageVersion\.of\((\d+)\)",
    )
    .ok()
    .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // targetCompatibility as fallback
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("targetCompatibility") {
            if let Some(version) = trimmed.split(['=', '(', ')', ' ']).find(|s| {
                let s = s.trim();
                !s.is_empty() && (s.chars().all(|c| c.is_ascii_digit()) || s.contains("VERSION_"))
            }) {
                let version_num = version
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .replace("JavaVersion.VERSION_", "")
                    .replace('_', ".");

                let version_final = if version_num.starts_with("1.") && version_num.len() > 2 {
                    version_num.get(2..).unwrap_or(&version_num).to_string()
                } else {
                    version_num
                };

                return Some(version_final);
            }
        }
    }

    None
}

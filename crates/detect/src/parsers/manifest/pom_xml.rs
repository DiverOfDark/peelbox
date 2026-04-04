use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_core::output::schema::{CopySpec, UniversalBuild};
use peelbox_wolfi::WolfiPackageIndex;
use regex::Regex;
use std::path::Path;
use tracing::debug;

const JAVA: LanguageId = LanguageId::new("java");
const MAVEN: BuildSystemId = BuildSystemId::new("maven");
const JVM: RuntimeId = RuntimeId::new("jvm");

inventory::submit! {
    LanguageMeta { slug: "java", display_name: "Java", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "maven", display_name: "Maven", aliases: &["maven"] }
}
inventory::submit! {
    RuntimeMeta { slug: "jvm", display_name: "JVM", aliases: &["java", "kotlin", "scala"] }
}

pub struct PomXmlParser;

impl ManifestParser for PomXmlParser {
    fn filenames(&self) -> &[&str] {
        &["pom.xml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("<project") && !content.contains("<artifactId>") {
            return None;
        }

        let doc = roxmltree::Document::parse(content).ok()?;
        let root = doc.root_element();

        let mut artifact_id = None;
        let mut version = None;
        let mut packaging = None;
        let mut parent_version = None;
        let mut modules = Vec::new();

        for child in root.children() {
            if child.has_tag_name("artifactId") && artifact_id.is_none() {
                artifact_id = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("version") {
                version = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("packaging") {
                packaging = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("parent") {
                for pc in child.children() {
                    if pc.has_tag_name("version") {
                        parent_version = pc.text().map(|s| s.trim().to_string());
                    }
                }
            }
            if child.has_tag_name("modules") {
                for mc in child.children() {
                    if mc.has_tag_name("module") {
                        if let Some(text) = mc.text() {
                            modules.push(text.trim().to_string());
                        }
                    }
                }
            }
        }

        let name = artifact_id?;
        let effective_version = version.or(parent_version);
        let is_pom = packaging.as_deref() == Some("pom");
        let is_application = !is_pom;

        let workspace = if !modules.is_empty() {
            Some(Workspace {
                members: modules,
                orchestrator: None,
            })
        } else {
            None
        };

        let dependencies = parse_maven_deps(&doc);

        let java_version = detect_java_version_from_pom(content);

        // Construct Docker Hub build image: maven:3-eclipse-temurin-{jdk}
        let jdk = java_version.as_deref().unwrap_or("21");
        let build_image = format!("docker.io/library/maven:3-eclipse-temurin-{}", jdk);

        let has_spring_boot_plugin = content.contains("spring-boot-maven-plugin");

        // Detect Maven wrapper (mvnw) — walk up from pom.xml looking for mvnw
        // in parent Maven project directories (handles multimodule layouts)
        let has_wrapper = find_mvnw_in_ancestors(path);
        let mvn_cmd = if has_wrapper { "./mvnw" } else { "mvn" };

        let entrypoint = if has_spring_boot_plugin {
            let jar_name = match &effective_version {
                Some(ver) => format!("/app/{}-{}.jar", name, ver),
                None => format!("/app/{}.jar", name),
            };
            Some(format!("java -jar {}", jar_name))
        } else {
            extract_main_class(&doc).map(|mc| format!("java -cp /app/*:/app/lib/* {}", mc))
        };

        let java_pkg = java_version
            .as_ref()
            .map(|v| format!("openjdk-{}", v))
            .unwrap_or_else(|| "openjdk".into());

        let target_dir = "target".to_string();

        Some(Manifest {
            path: path.to_path_buf(),
            language: JAVA,
            build_system: MAVEN,
            runtime: JVM,
            package: Some(Package {
                name: name.clone(),
                version: effective_version,
                is_application,
            }),
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec![java_pkg.clone(), "maven".into(), "ca-certificates".into()],
                commands: vec![
                    format!("{} package -DskipTests", mvn_cmd),
                    format!(
                        "{} dependency:copy-dependencies -DoutputDirectory={}/lib",
                        mvn_cmd, target_dir
                    ),
                ],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        format!("{} -pl {{module}} -am install -DskipTests", mvn_cmd),
                        format!(
                            "{} -pl {{module}} dependency:copy-dependencies -DoutputDirectory=target/lib",
                            mvn_cmd
                        ),
                    ],
                    member_artifacts: Some(vec![
                        ("{module}/target/*.jar".into(), "/app/".into()),
                        ("{module}/target/lib/".into(), "/app/lib".into()),
                    ]),
                }),
                env: btree(&[
                    (
                        "JAVA_HOME",
                        &format!(
                            "/usr/lib/jvm/java-{}-openjdk",
                            java_version.as_deref().unwrap_or("21")
                        ),
                    ),
                    ("MAVEN_OPTS", "-Dmaven.repo.local=/root/.m2/repository"),
                ]),
                cache_dirs: vec!["/root/.m2/repository".into()],
                artifacts: vec![
                    (format!("{}/*.jar", target_dir), "/app/".into()),
                    (format!("{}/lib/", target_dir), "/app/lib".into()),
                ],
                build_image: Some(build_image),
                asset_build: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    java_version
                        .as_ref()
                        .map(|v| format!("openjdk-{}-jre", v))
                        .unwrap_or_else(|| "openjdk".into()),
                    "ca-certificates".into(),
                ],
                env: {
                    let java_home = format!(
                        "/usr/lib/jvm/java-{}-openjdk",
                        java_version.as_deref().unwrap_or("21")
                    );
                    btree(&[
                        ("CLASSPATH", "/app/*:/app/lib/*"),
                        ("JAVA_HOME", &java_home),
                        (
                            "PATH",
                            &format!(
                                "{}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                                java_home
                            ),
                        ),
                    ])
                },
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

fn parse_maven_deps(doc: &roxmltree::Document) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for node in doc.descendants() {
        if node.has_tag_name("dependency") {
            let mut group_id = None;
            let mut artifact_id = None;
            let mut ver = None;
            let mut scope_str = None;

            for child in node.children() {
                if child.has_tag_name("groupId") {
                    group_id = child.text().map(|s| s.trim().to_string());
                }
                if child.has_tag_name("artifactId") {
                    artifact_id = child.text().map(|s| s.trim().to_string());
                }
                if child.has_tag_name("version") {
                    ver = child.text().map(|s| s.trim().to_string());
                }
                if child.has_tag_name("scope") {
                    scope_str = child.text().map(|s| s.trim().to_string());
                }
            }

            if let (Some(gid), Some(aid)) = (group_id, artifact_id) {
                let scope = match scope_str.as_deref() {
                    Some("test") => DepScope::Dev,
                    Some("provided") => DepScope::Build,
                    _ => DepScope::Runtime,
                };
                deps.push(Dependency {
                    name: format!("{}:{}", gid, aid),
                    version: ver,
                    scope,
                    is_internal: false,
                });
            }
        }
    }
    deps
}

fn detect_java_version_from_pom(content: &str) -> Option<String> {
    detect_java_version(content)
}

/// Walk up from a pom.xml looking for mvnw in ancestor directories.
/// Stops when the parent no longer looks like a Maven project
/// (i.e., has no pom.xml or .mvn directory).
fn find_mvnw_in_ancestors(pom_path: &Path) -> bool {
    let mut dir = pom_path.parent();
    while let Some(d) = dir {
        if d.join("mvnw").exists() {
            return true;
        }
        // Only continue up if the parent looks like part of a Maven project hierarchy
        dir = d
            .parent()
            .filter(|p| p.join("pom.xml").exists() || p.join(".mvn").exists());
    }
    false
}

fn extract_main_class(doc: &roxmltree::Document) -> Option<String> {
    for node in doc.descendants() {
        if node.has_tag_name("mainClass") {
            return node.text().map(|s| s.trim().to_string());
        }
    }
    None
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PomXmlParser))
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["Java", "Kotlin"],
        extensions: &["java", "kt", "kts", "properties", "yml", "yaml"],
        port_patterns: &[
            r"\.setPort\(\s*(\d{4,5})\s*\)",
            r#"server\.port\s*=\s*(\d{4,5})"#,
        ],
        health_patterns: &[r#"@(?:Get|Request)Mapping\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
        env_var_patterns: &[r#"System\.getenv\(["']([A-Z_][A-Z0-9_]*)"#],
    }
}

// ── Build System Profile ────────────────────────────────────────────────────

fn maven_subdirectory_command(cmd: &str, subdir: &str) -> String {
    let (prefix, rest) = if let Some(rest) = cmd.strip_prefix("./mvnw ") {
        ("./mvnw", rest)
    } else if let Some(rest) = cmd.strip_prefix("mvn ") {
        ("mvn", rest)
    } else {
        return format!("cd {} && {}", subdir, cmd);
    };
    let mut result = format!("{} -f {}/pom.xml {}", prefix, subdir, rest);
    // For dependency:copy-dependencies, ensure the target dir exists
    if cmd.contains("dependency:copy-dependencies") {
        result = format!("{} && mkdir -p {}/target/lib", result, subdir);
    }
    result
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        transform_subdirectory_command: maven_subdirectory_command,
        ..BuildSystemConfig::new(MAVEN)
    })
}

// ── Read Java version from version files ────────────────────────────────

/// Read Java version from `.java-version` or `.sdkmanrc` files in the project
/// directory or repo root. Returns the normalized major version (e.g., "17", "21").
pub(crate) fn read_java_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    for dir in &[project_dir, repo_root] {
        // 1. .java-version file (plain text version number)
        let java_version_file = dir.join(".java-version");
        if let Ok(content) = std::fs::read_to_string(&java_version_file) {
            if let Some(version) = parse_java_version_file(&content) {
                return Some(normalize_java_version(&version));
            }
        }

        // 2. .sdkmanrc file (SDKMAN configuration)
        let sdkmanrc = dir.join(".sdkmanrc");
        if let Ok(content) = std::fs::read_to_string(&sdkmanrc) {
            if let Some(version) = parse_sdkmanrc_java_version(&content) {
                return Some(normalize_java_version(&version));
            }
        }
    }

    None
}

/// Parse Java version from .sdkmanrc file.
/// Format: `java=17.0.9-tem` or `java=21.0.1-ms`
fn parse_sdkmanrc_java_version(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("java=") {
            // Take the major version number (before first dot or dash)
            let version: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }
    None
}

/// Construct a Maven Docker Hub image tag for a given JDK version.
/// Used by `scan_version_files` to update the build image when a `.java-version`
/// file overrides the JDK version detected from pom.xml.
pub(crate) fn maven_build_image_for_version(jdk_version: &str) -> String {
    format!("docker.io/library/maven:3-eclipse-temurin-{}", jdk_version)
}

// ── Shared Java/Kotlin version detection and Wolfi resolution ───────────

/// Minimum Java version that is expected to always be in Wolfi.
const MIN_GUARANTEED_WOLFI_JAVA_VERSION: u32 = 7;

/// Normalize old-style Java 1.x version strings to their modern equivalent.
/// "1.8" → "8", "1.7" → "7", "17" → "17"
pub(crate) fn normalize_java_version(version: &str) -> String {
    let trimmed = version.trim();

    if let Some(rest) = trimmed.strip_prefix("1.") {
        let minor: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !minor.is_empty() {
            if let Ok(v) = minor.parse::<u32>() {
                if v <= 8 {
                    return minor;
                }
            }
        }
    }

    let major: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !major.is_empty() {
        return major;
    }

    trimmed.to_string()
}

/// Detect Java version from manifest content (pom.xml, build.gradle, .java-version).
/// Returns the normalized version number (e.g., "17", "21", "11", "8").
pub(crate) fn detect_java_version(manifest_content: &str) -> Option<String> {
    // Try pom.xml patterns first
    if let Some(ver) = parse_pom_version(manifest_content) {
        return Some(normalize_java_version(&ver));
    }

    // Try build.gradle(.kts) patterns
    if let Some(ver) = super::build_gradle::parse_gradle_version(manifest_content) {
        return Some(normalize_java_version(&ver));
    }

    // Try .java-version file (plain text version number)
    parse_java_version_file(manifest_content).map(|v| normalize_java_version(&v))
}

/// Parse Java version from pom.xml content using XML parsing.
pub(crate) fn parse_pom_version(content: &str) -> Option<String> {
    if !content.contains('<') {
        return None;
    }

    if let Ok(doc) = roxmltree::Document::parse(content) {
        for node in doc.descendants() {
            if node.has_tag_name("maven.compiler.source")
                || node.has_tag_name("java.version")
                || node.has_tag_name("maven.compiler.release")
            {
                if let Some(version) = node.text() {
                    return Some(version.trim().to_string());
                }
            }
        }
    }

    // Fallback to regex for cases where XML parsing fails
    if let Some(caps) = Regex::new(r"<maven\.compiler\.source>([\d.]+)</maven\.compiler\.source>")
        .ok()
        .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    if let Some(caps) = Regex::new(r"<java\.version>([\d.]+)</java\.version>")
        .ok()
        .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    if let Some(caps) = Regex::new(r"<release>([\d.]+)</release>")
        .ok()
        .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    None
}

/// Parse version from a .java-version file (plain text version number).
fn parse_java_version_file(content: &str) -> Option<String> {
    if content.contains('<') || content.contains('{') {
        return None;
    }
    let trimmed = content.trim();
    if Regex::new(r"^\d+(\.\d+)*$").ok()?.is_match(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Returns the JAVA_HOME path for a given Java major version as installed by Wolfi.
pub(crate) fn wolfi_java_home_path(major_version: &str) -> String {
    match major_version.parse::<u32>() {
        Ok(v) if v <= 9 => format!("/usr/lib/jvm/java-1.{}-openjdk", v),
        _ => format!("/usr/lib/jvm/java-{}-openjdk", major_version),
    }
}

/// After Wolfi package resolution, synchronize JAVA_HOME and PATH env vars
/// with the actual resolved openjdk package version.
pub(crate) fn sync_java_home_with_packages(build: &mut UniversalBuild) {
    let lang = &build.metadata.language;
    if lang != "Java" && lang != "Kotlin" && lang != "Scala" && lang != "Clojure" {
        return;
    }

    let java_version = build
        .build
        .packages
        .iter()
        .chain(build.runtime.packages.iter())
        .find_map(|pkg| {
            pkg.strip_prefix("openjdk-").and_then(|rest| {
                let version = rest.split('-').next().unwrap_or(rest);
                if version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    Some(version.to_string())
                } else {
                    None
                }
            })
        });

    let version = match java_version {
        Some(v) => v,
        None => return,
    };

    let correct_java_home = wolfi_java_home_path(&version);
    let correct_path = format!(
        "{}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        correct_java_home
    );

    if let Some(current) = build.build.env.get("JAVA_HOME") {
        if *current != correct_java_home {
            debug!(
                from = %current,
                to = %correct_java_home,
                "Correcting build JAVA_HOME to match resolved package"
            );
            build
                .build
                .env
                .insert("JAVA_HOME".to_string(), correct_java_home.clone());
            if build.build.env.contains_key("PATH") {
                build
                    .build
                    .env
                    .insert("PATH".to_string(), correct_path.clone());
            }
        }
    }

    if let Some(current) = build.runtime.env.get("JAVA_HOME") {
        if *current != correct_java_home {
            debug!(
                from = %current,
                to = %correct_java_home,
                "Correcting runtime JAVA_HOME to match resolved package"
            );
            build
                .runtime
                .env
                .insert("JAVA_HOME".to_string(), correct_java_home.clone());
            if build.runtime.env.contains_key("PATH") {
                build.runtime.env.insert("PATH".to_string(), correct_path);
            }
        }
    }
}

/// When a Java version is not available in Wolfi, replace openjdk packages
/// with an Adoptium/Temurin JDK download.
pub(crate) fn resolve_java_toolchain(build: &mut UniversalBuild, wolfi: &WolfiPackageIndex) {
    let lang = &build.metadata.language;
    if lang != "Java" && lang != "Kotlin" && lang != "Scala" && lang != "Clojure" {
        return;
    }

    let legacy_version = find_legacy_openjdk_version(&build.build.packages, wolfi)
        .or_else(|| find_legacy_openjdk_version(&build.runtime.packages, wolfi));

    let version = match legacy_version {
        Some(v) => v,
        None => return,
    };

    debug!(
        version = %version,
        "Java version not available in Wolfi, switching to Adoptium Temurin",
    );

    let java_home = format!("/usr/lib/jvm/java-{}", version);
    let java_home_bin = format!("{}/bin", java_home);
    let install_cmd = format!(
        "curl -fsSL https://api.adoptium.net/v3/binary/latest/{version}/ga/linux/x64/jdk/hotspot/normal/eclipse \
         -o /tmp/jdk.tar.gz && \
         mkdir -p {java_home} && \
         tar -xzf /tmp/jdk.tar.gz -C {java_home} --strip-components=1 && \
         rm /tmp/jdk.tar.gz",
        version = version,
        java_home = java_home,
    );

    replace_legacy_openjdk_packages(&mut build.build.packages, &version);
    ensure_package(&mut build.build.packages, "curl");
    ensure_package(&mut build.build.packages, "ca-certificates");

    build.build.commands.insert(0, install_cmd);

    build
        .build
        .env
        .insert("JAVA_HOME".to_string(), java_home.clone());
    let path_value = format!(
        "{}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        java_home_bin
    );
    build.build.env.insert("PATH".to_string(), path_value);

    replace_legacy_openjdk_packages(&mut build.runtime.packages, &version);

    build.runtime.copy.push(CopySpec {
        from: format!("{}/", java_home),
        to: format!("{}/", java_home),
    });

    build
        .runtime
        .env
        .insert("JAVA_HOME".to_string(), java_home.clone());
    build.runtime.env.insert(
        "PATH".to_string(),
        format!(
            "{}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            java_home_bin
        ),
    );
}

fn find_legacy_openjdk_version(packages: &[String], wolfi: &WolfiPackageIndex) -> Option<String> {
    for pkg in packages {
        let version_str = if let Some(v) = pkg.strip_prefix("openjdk-") {
            v.split('-').next().unwrap_or(v)
        } else {
            continue;
        };

        if let Ok(ver) = version_str.parse::<u32>() {
            if ver >= MIN_GUARANTEED_WOLFI_JAVA_VERSION {
                continue;
            }
            let base_pkg = format!("openjdk-{}", version_str);
            if !wolfi.has_package(&base_pkg) {
                return Some(version_str.to_string());
            }
        }
    }
    None
}

fn replace_legacy_openjdk_packages(packages: &mut Vec<String>, version: &str) {
    packages.retain(|pkg| {
        if let Some(v) = pkg.strip_prefix("openjdk-") {
            let pkg_version = v.split('-').next().unwrap_or(v);
            pkg_version != version
        } else {
            true
        }
    });
}

fn ensure_package(packages: &mut Vec<String>, pkg: &str) {
    if !packages.iter().any(|p| p == pkg) {
        packages.push(pkg.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    const POM_CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
    <properties>
        <maven.compiler.source>17</maven.compiler.source>
        <maven.compiler.target>17</maven.compiler.target>
    </properties>
    <build>
        <plugins>
            <plugin>
                <artifactId>spring-boot-maven-plugin</artifactId>
            </plugin>
        </plugins>
    </build>
</project>"#;

    #[test]
    fn test_pom_without_wrapper_uses_mvn() {
        let dir = tempfile::tempdir().unwrap();
        let pom_path = dir.path().join("pom.xml");
        std::fs::write(&pom_path, POM_CONTENT).unwrap();

        let manifest = PomXmlParser.parse(&pom_path, POM_CONTENT).unwrap();
        assert_eq!(manifest.build.commands[0], "mvn package -DskipTests");
        assert!(manifest.build.commands[1].starts_with("mvn dependency:copy-dependencies"));
    }

    #[test]
    fn test_pom_with_wrapper_uses_mvnw() {
        let dir = tempfile::tempdir().unwrap();
        let pom_path = dir.path().join("pom.xml");
        std::fs::write(&pom_path, POM_CONTENT).unwrap();
        // Create mvnw wrapper script
        std::fs::write(dir.path().join("mvnw"), "#!/bin/sh\n").unwrap();

        let manifest = PomXmlParser.parse(&pom_path, POM_CONTENT).unwrap();
        assert_eq!(manifest.build.commands[0], "./mvnw package -DskipTests");
        assert_eq!(
            manifest.build.commands[1],
            "./mvnw dependency:copy-dependencies -DoutputDirectory=target/lib"
        );

        // Check member_transform commands preserve {module} placeholder
        let transform = manifest.build.member_transform.unwrap();
        assert_eq!(
            transform.member_commands[0],
            "./mvnw -pl {module} -am install -DskipTests"
        );
        assert_eq!(
            transform.member_commands[1],
            "./mvnw -pl {module} dependency:copy-dependencies -DoutputDirectory=target/lib"
        );
    }

    #[test]
    fn test_pom_in_submodule_finds_wrapper_at_root() {
        let dir = tempfile::tempdir().unwrap();
        // Root has mvnw and root pom.xml
        std::fs::write(dir.path().join("mvnw"), "#!/bin/sh\n").unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            "<project><modules><module>api</module></modules></project>",
        )
        .unwrap();
        // Submodule pom.xml
        let sub = dir.path().join("api");
        std::fs::create_dir_all(&sub).unwrap();
        let sub_pom = sub.join("pom.xml");
        std::fs::write(&sub_pom, POM_CONTENT).unwrap();

        let manifest = PomXmlParser.parse(&sub_pom, POM_CONTENT).unwrap();
        assert_eq!(manifest.build.commands[0], "./mvnw package -DskipTests");
    }

    // ── Java version detection tests ────────────────────────────────────

    #[test]
    fn test_pom_maven_compiler_source() {
        let content = r#"<project><properties><maven.compiler.source>17</maven.compiler.source></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("17".to_string()));
    }

    #[test]
    fn test_pom_java_version() {
        let content =
            r#"<project><properties><java.version>21</java.version></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("21".to_string()));
    }

    #[test]
    fn test_pom_maven_compiler_release() {
        let content = r#"<project><properties><maven.compiler.release>11</maven.compiler.release></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("11".to_string()));
    }

    #[test]
    fn test_gradle_source_compat() {
        let content = r#"sourceCompatibility = "17""#;
        assert_eq!(detect_java_version(content), Some("17".to_string()));
    }

    #[test]
    fn test_gradle_toolchain() {
        let content = r#"java { toolchain { languageVersion.set(JavaLanguageVersion.of(21)) } }"#;
        assert_eq!(detect_java_version(content), Some("21".to_string()));
    }

    #[test]
    fn test_java_version_file() {
        assert_eq!(detect_java_version("17"), Some("17".to_string()));
        assert_eq!(detect_java_version("17.0"), Some("17".to_string()));
    }

    #[test]
    fn test_java_version_file_legacy() {
        assert_eq!(detect_java_version("1.8"), Some("8".to_string()));
        assert_eq!(detect_java_version("1.8.0"), Some("8".to_string()));
    }

    #[test]
    fn test_pom_legacy_java_8() {
        let content = r#"<project><properties><maven.compiler.source>1.8</maven.compiler.source></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("8".to_string()));
    }

    #[test]
    fn test_pom_legacy_java_7() {
        let content =
            r#"<project><properties><java.version>1.7</java.version></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("7".to_string()));
    }

    #[test]
    fn test_gradle_legacy_version_1_8() {
        let content = r#"sourceCompatibility = JavaVersion.VERSION_1_8"#;
        assert_eq!(detect_java_version(content), Some("8".to_string()));
    }

    #[test]
    fn test_normalize_1_8() {
        assert_eq!(normalize_java_version("1.8"), "8");
    }

    #[test]
    fn test_normalize_1_7() {
        assert_eq!(normalize_java_version("1.7"), "7");
    }

    #[test]
    fn test_normalize_modern_17() {
        assert_eq!(normalize_java_version("17"), "17");
    }

    #[test]
    fn test_no_version() {
        assert_eq!(detect_java_version("<project></project>"), None);
    }

    #[test]
    fn test_pom_version_standalone() {
        let content = r#"<project><properties><maven.compiler.source>21</maven.compiler.source></properties></project>"#;
        assert_eq!(parse_pom_version(content), Some("21".to_string()));
    }

    #[test]
    fn test_gradle_version_standalone() {
        let content = r#"sourceCompatibility = "17""#;
        assert_eq!(
            crate::parsers::manifest::build_gradle::parse_gradle_version(content),
            Some("17".to_string())
        );
    }

    #[test]
    fn test_gradle_version_version_prefix() {
        let content = r#"sourceCompatibility = JavaVersion.VERSION_17"#;
        assert_eq!(
            crate::parsers::manifest::build_gradle::parse_gradle_version(content),
            Some("17".to_string())
        );
    }
}

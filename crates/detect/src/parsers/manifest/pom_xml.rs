use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

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
                            setup_commands: vec![],
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
    crate::version::java::detect_java_version(content)
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
}

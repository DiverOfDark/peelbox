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
    RuntimeMeta { slug: "jvm", display_name: "JVM", aliases: &["java", "kotlin"] }
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

        // Detect Maven wrapper (mvnw) — prefer over system Maven when present
        let has_wrapper = path
            .parent()
            .map(|dir| dir.join("mvnw").exists())
            .unwrap_or(false);
        let mvn_cmd = if has_wrapper { "./mvnw" } else { "mvn" };

        let entrypoint = if has_spring_boot_plugin {
            let jar_name = match &effective_version {
                Some(ver) => format!("/app/{}-{}.jar", name, ver),
                None => format!("/app/{}.jar", name),
            };
            Some(format!("java -jar {}", jar_name))
        } else {
            extract_main_class(&doc).map(|mc| format!("java -cp /app/*.jar {}", mc))
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
        assert!(manifest.build.commands[1].starts_with("./mvnw dependency:copy-dependencies"));

        // Also check member_transform commands
        let transform = manifest.build.member_transform.unwrap();
        assert!(transform.member_commands[0].starts_with("./mvnw -pl"));
        assert!(transform.member_commands[1].starts_with("./mvnw -pl"));
    }
}

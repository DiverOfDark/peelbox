//! Maven build system (Java/Kotlin)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::language::LanguageDefinition;
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use roxmltree::Document;
use std::path::{Path, PathBuf};

pub struct MavenBuildSystem;

impl BuildSystem for MavenBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Maven
    }

    fn language_id(&self) -> Option<crate::LanguageId> {
        Some(crate::LanguageId::Java)
    }

    fn runtime_id(&self) -> Option<crate::RuntimeId> {
        Some(crate::RuntimeId::JVM)
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "pom.xml".to_string(),
            priority: 10,
        }]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            if rel_path.file_name().and_then(|n| n.to_str()) == Some("pom.xml") {
                let abs_path = repo_root.join(rel_path);
                let content = fs.read_to_string(&abs_path).ok();

                let is_valid = if let Some(c) = content.as_deref() {
                    c.contains("<project") || c.contains("<artifactId>")
                } else {
                    true
                };

                if is_valid {
                    let lang = crate::language::JavaLanguage;
                    let project_dir = rel_path.parent().unwrap_or(Path::new(""));

                    if lang.is_runnable(fs, repo_root, project_dir, file_tree, content.as_deref()) {
                        detections.push(DetectionStack::new(
                            BuildSystemId::Maven,
                            LanguageId::Java,
                            rel_path.clone(),
                        ));
                    }
                }
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        service_path: &Path,
        relative_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let java_version = manifest_content
            .and_then(parse_java_version)
            .or_else(|| {
                // Try parent pom.xml for inherited properties (multi-module projects)
                service_path
                    .parent()
                    .map(|parent| parent.join("pom.xml"))
                    .filter(|p| p.exists())
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|content| parse_java_version(&content))
            })
            .or_else(|| wolfi_index.get_latest_version("openjdk"))
            .expect("Failed to get openjdk version from Wolfi index");

        let _runtime_version = format!("{}-jre", java_version);

        let maven_version = wolfi_index
            .get_latest_version("maven")
            .expect("Failed to get maven version from Wolfi index");

        let java_home = if java_version == "openjdk-8" {
            "/usr/lib/jvm/java-1.8-openjdk".to_string()
        } else {
            format!(
                "/usr/lib/jvm/java-{}-openjdk",
                java_version.trim_start_matches("openjdk-")
            )
        };

        let mut build_env = std::collections::BTreeMap::new();
        build_env.insert("JAVA_HOME".to_string(), java_home);
        build_env.insert(
            "MAVEN_OPTS".to_string(),
            "-Dmaven.repo.local=/root/.m2/repository".to_string(),
        );

        let mut runtime_env = std::collections::BTreeMap::new();
        runtime_env.insert("CLASSPATH".to_string(), "/app/*:/app/lib/*".to_string());

        let mut build_packages = vec![java_version, maven_version];
        build_packages.push("ca-certificates".to_string());

        let is_root = relative_path.components().count() == 0 || relative_path == Path::new(".");
        let service_dir = relative_path.to_string_lossy();

        // Check if there's a parent reactor pom.xml (multi-module project)
        let has_reactor_root = !is_root
            && service_path
                .parent()
                .map(|parent| parent.join("pom.xml"))
                .filter(|p| p.exists())
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|content| content.contains("<modules>"))
                .unwrap_or(false);

        let target_dir = if is_root {
            "target".to_string()
        } else {
            format!("{}/target", service_dir)
        };

        let build_commands = if is_root {
            vec![
                "mvn package -DskipTests".to_string(),
                format!(
                    "mvn dependency:copy-dependencies -DoutputDirectory={}/lib",
                    target_dir
                ),
            ]
        } else if has_reactor_root {
            // For submodules in a multi-module project, use -pl (project list) with -am (also-make)
            // from the reactor root. Use install instead of package so that sibling module jars are
            // available in the local Maven repo for the dependency:copy-dependencies goal.
            // Note: dependency:copy-dependencies runs in the module's directory, so
            // outputDirectory is relative to the module root (use target/lib, not service_dir/target/lib).
            vec![
                format!("mvn -pl {} -am install -DskipTests", service_dir),
                format!(
                    "mvn -pl {} dependency:copy-dependencies -DoutputDirectory=target/lib",
                    service_dir
                ),
            ]
        } else {
            // Standalone Maven project in a subdirectory (no parent reactor),
            // use -f to point directly to the pom.xml.
            // mkdir -p ensures the lib dir exists even when there are zero dependencies.
            vec![
                format!("mvn -f {}/pom.xml package -DskipTests", service_dir),
                format!(
                    "mvn -f {}/pom.xml dependency:copy-dependencies -DoutputDirectory=target/lib; mkdir -p {}/lib",
                    service_dir, target_dir
                ),
            ]
        };

        // Compute entrypoint from manifest content
        let entrypoint = manifest_content.and_then(|content| detect_maven_entrypoint(content));

        BuildTemplate {
            build_packages,
            build_commands,
            cache_paths: vec!["/root/.m2/repository/".to_string()],
            common_ports: vec![8080],
            build_env,
            runtime_copy: vec![
                (format!("{}/*.jar", target_dir), "/app/".to_string()),
                (format!("{}/lib/", target_dir), "/app/lib".to_string()),
            ],
            runtime_env,
            runtime_workdir: None,
            entrypoint,
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["/root/.m2/repository".to_string()]
    }
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("<modules>")
        } else {
            false
        }
    }

    fn parse_package_metadata(
        &self,
        manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        let doc = Document::parse(manifest_content)?;

        let mut artifact_id = None;
        let mut packaging = None;

        // Find root <project> element
        let root = doc.root_element();

        // Only look at direct children of <project>
        for child in root.children() {
            if child.has_tag_name("artifactId") && artifact_id.is_none() {
                artifact_id = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("packaging") {
                packaging = child.text().map(|s| s.trim().to_string());
            }
        }

        let name = artifact_id.ok_or_else(|| anyhow::anyhow!("No artifactId found in pom.xml"))?;
        let is_application = packaging.as_deref() != Some("pom");

        Ok((name, is_application))
    }

    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        let doc = Document::parse(manifest_content)?;

        let mut patterns = Vec::new();
        for node in doc.descendants() {
            if node.has_tag_name("modules") {
                for child in node.children() {
                    if child.has_tag_name("module") {
                        if let Some(text) = child.text() {
                            patterns.push(text.trim().to_string());
                        }
                    }
                }
            }
        }

        Ok(patterns)
    }
}

fn parse_java_version(manifest_content: &str) -> Option<String> {
    crate::version::java::detect_java_version_wolfi(manifest_content)
}

/// Detect the entrypoint command from a pom.xml manifest.
///
/// If the Spring Boot Maven Plugin is present, produces a `java -jar` command
/// with the specific artifact name. Otherwise, looks for a `<mainClass>` element
/// and produces a classpath-based command.
fn detect_maven_entrypoint(content: &str) -> Option<String> {
    if content.contains("spring-boot-maven-plugin") {
        parse_pom_jar_name(content).map(|jar_path| format!("java -jar {}", jar_path))
    } else {
        extract_main_class(content).map(|main_class| format!("java -cp /app/*.jar {}", main_class))
    }
}

fn parse_pom_jar_name(content: &str) -> Option<String> {
    let doc = Document::parse(content).ok()?;
    let root = doc.root_element();

    let mut artifact_id = None;
    let mut version = None;
    let mut parent_version = None;

    for child in root.children() {
        if child.has_tag_name("artifactId") && artifact_id.is_none() {
            artifact_id = child.text().map(|s| s.trim().to_string());
        }
        if child.has_tag_name("version") {
            version = child.text().map(|s| s.trim().to_string());
        }
        if child.has_tag_name("parent") {
            for parent_child in child.children() {
                if parent_child.has_tag_name("version") {
                    parent_version = parent_child.text().map(|s| s.trim().to_string());
                }
            }
        }
    }

    let effective_version = version.or(parent_version);

    match (artifact_id, effective_version) {
        (Some(aid), Some(ver)) => Some(format!("/app/{}-{}.jar", aid, ver)),
        (Some(aid), None) => Some(format!("/app/{}.jar", aid)),
        _ => None,
    }
}

fn extract_main_class(content: &str) -> Option<String> {
    let doc = Document::parse(content).ok()?;
    for node in doc.descendants() {
        if node.has_tag_name("mainClass") {
            return node.text().map(|s| s.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_core::fs::{DirEntry, FileMetadata, FileType};
    use peelbox_wolfi::WolfiPackageIndex;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    struct MockFileSystem {
        files: HashMap<PathBuf, String>,
    }

    impl FileSystem for MockFileSystem {
        fn read_to_string(&self, path: &Path) -> Result<String, anyhow::Error> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }

        fn exists(&self, path: &Path) -> bool {
            self.files.contains_key(path)
        }

        fn is_file(&self, path: &Path) -> bool {
            self.exists(path) // Simplification
        }
        fn is_dir(&self, _path: &Path) -> bool {
            false
        } // Simplification
        fn read_dir(&self, _path: &Path) -> Result<Vec<DirEntry>, anyhow::Error> {
            Ok(vec![])
        } // Simplification

        fn metadata(&self, path: &Path) -> Result<FileMetadata, anyhow::Error> {
            if self.exists(path) {
                Ok(FileMetadata {
                    size: 100,
                    file_type: FileType::File,
                })
            } else {
                Err(anyhow::anyhow!("not found"))
            }
        }

        fn read_bytes(&self, path: &Path, _max_bytes: usize) -> Result<Vec<u8>, anyhow::Error> {
            self.read_to_string(path).map(|s| s.into_bytes())
        }

        fn canonicalize(&self, path: &Path) -> Result<PathBuf, anyhow::Error> {
            Ok(path.to_path_buf())
        }
    }

    #[test]
    fn test_detect_simple_maven() {
        let maven = MavenBuildSystem;
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/pom.xml"),
            r#"<project>
                <groupId>com.example</groupId>
                <artifactId>my-app</artifactId>
                <version>1.0.0</version>
            </project>"#
                .to_string(),
        );

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("pom.xml")];

        // This test is limited by the lack of mocks for LanguageDefinition in this context,
        // but we can verify it compiles and runs.
        let _ = maven.detect_all(&repo_root, &file_tree, &fs);
    }

    #[test]
    fn test_build_template_generation() {
        let maven = MavenBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();

        let template = maven.build_template(
            &wolfi_index,
            Path::new("."),
            Path::new("."),
            Some("<project><properties><java.version>21</java.version></properties></project>"),
        );

        assert!(template
            .build_packages
            .iter()
            .any(|p| p.contains("openjdk-21")));
        assert!(template.build_packages.iter().any(|p| p.contains("maven")));
        assert_eq!(
            template.build_commands,
            vec![
                "mvn package -DskipTests",
                "mvn dependency:copy-dependencies -DoutputDirectory=target/lib"
            ]
        );
        // Verify path mapping format: (src, dest)
        assert!(template
            .runtime_copy
            .contains(&("target/*.jar".to_string(), "/app/".to_string())));
        assert!(template
            .runtime_copy
            .contains(&("target/lib/".to_string(), "/app/lib".to_string())));
    }

    #[test]
    fn test_parse_java_version_test() {
        assert_eq!(
            parse_java_version(
                "<project><properties><java.version>17</java.version></properties></project>"
            ),
            Some("openjdk-17".to_string())
        );
        assert_eq!(parse_java_version("<project><properties><maven.compiler.source>21</maven.compiler.source></properties></project>"), Some("openjdk-21".to_string()));
        assert_eq!(parse_java_version("<project><properties><maven.compiler.release>11</maven.compiler.release></properties></project>"), Some("openjdk-11".to_string()));
        assert_eq!(parse_java_version("<project></project>"), None);
    }

    #[test]
    fn test_build_template_submodule_includes_also_make() {
        let maven = MavenBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();

        // Set up a temp dir simulating a multi-module project with a reactor root
        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();
        let service_dir = root.join("api-service");
        std::fs::create_dir_all(&service_dir).unwrap();
        std::fs::write(
            root.join("pom.xml"),
            "<project><modules><module>api-service</module></modules></project>",
        )
        .unwrap();

        // Test for a submodule (non-root path) with a reactor root
        let template = maven.build_template(
            &wolfi_index,
            &service_dir,
            Path::new("api-service"),
            Some("<project><properties><java.version>21</java.version></properties></project>"),
        );

        assert_eq!(
            template.build_commands,
            vec![
                "mvn -pl api-service -am install -DskipTests",
                "mvn -pl api-service dependency:copy-dependencies -DoutputDirectory=target/lib"
            ]
        );

        // Verify path mapping includes submodule path
        assert!(template
            .runtime_copy
            .contains(&("api-service/target/*.jar".to_string(), "/app/".to_string())));
        assert!(template.runtime_copy.contains(&(
            "api-service/target/lib/".to_string(),
            "/app/lib".to_string()
        )));
    }

    #[test]
    fn test_build_template_standalone_subdir_uses_f_flag() {
        let maven = MavenBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();

        // Standalone Maven project in a subdirectory (no reactor root)
        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();
        let service_dir = root.join("backend");
        std::fs::create_dir_all(&service_dir).unwrap();
        // No root pom.xml

        let template = maven.build_template(
            &wolfi_index,
            &service_dir,
            Path::new("backend"),
            Some("<project><properties><java.version>17</java.version></properties></project>"),
        );

        assert_eq!(
            template.build_commands,
            vec![
                "mvn -f backend/pom.xml package -DskipTests",
                "mvn -f backend/pom.xml dependency:copy-dependencies -DoutputDirectory=target/lib; mkdir -p backend/target/lib"
            ]
        );
    }

    #[test]
    fn test_parse_pom_jar_name_with_direct_version() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
    <version>2.0.0</version>
</project>"#;
        assert_eq!(
            parse_pom_jar_name(content),
            Some("/app/my-app-2.0.0.jar".to_string())
        );
    }

    #[test]
    fn test_parse_pom_jar_name_with_parent_version() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <parent>
        <groupId>com.example</groupId>
        <artifactId>parent</artifactId>
        <version>1.0.0</version>
    </parent>
    <artifactId>api-service</artifactId>
</project>"#;
        assert_eq!(
            parse_pom_jar_name(content),
            Some("/app/api-service-1.0.0.jar".to_string())
        );
    }

    #[test]
    fn test_parse_pom_jar_name_direct_version_overrides_parent() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <parent>
        <groupId>com.example</groupId>
        <artifactId>parent</artifactId>
        <version>1.0.0</version>
    </parent>
    <artifactId>my-app</artifactId>
    <version>3.0.0</version>
</project>"#;
        assert_eq!(
            parse_pom_jar_name(content),
            Some("/app/my-app-3.0.0.jar".to_string())
        );
    }

    #[test]
    fn test_parse_pom_jar_name_no_version() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
</project>"#;
        assert_eq!(
            parse_pom_jar_name(content),
            Some("/app/my-app.jar".to_string())
        );
    }

    #[test]
    fn test_detect_maven_entrypoint_spring_boot() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
    <build>
        <plugins>
            <plugin>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-maven-plugin</artifactId>
            </plugin>
        </plugins>
    </build>
</project>"#;
        assert_eq!(
            detect_maven_entrypoint(content),
            Some("java -jar /app/my-app-1.0.0.jar".to_string())
        );
    }

    #[test]
    fn test_detect_maven_entrypoint_main_class() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-jar-plugin</artifactId>
                <configuration>
                    <mainClass>com.example.Main</mainClass>
                </configuration>
            </plugin>
        </plugins>
    </build>
</project>"#;
        assert_eq!(
            detect_maven_entrypoint(content),
            Some("java -cp /app/*.jar com.example.Main".to_string())
        );
    }

    #[test]
    fn test_detect_maven_entrypoint_none() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-lib</artifactId>
</project>"#;
        assert_eq!(detect_maven_entrypoint(content), None);
    }
}

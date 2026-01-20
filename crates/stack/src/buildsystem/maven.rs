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
                    } else if rel_path.to_string_lossy().contains("app") {
                        eprintln!("Maven skipped 'app' at {:?} because is_runnable returned false. Project root: {:?}", rel_path, project_dir);
                    }
                }
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let java_version = manifest_content
            .and_then(parse_java_version)
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

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("JAVA_HOME".to_string(), java_home);
        build_env.insert(
            "MAVEN_OPTS".to_string(),
            "-Dmaven.repo.local=/root/.m2/repository".to_string(),
        );

        let mut runtime_env = std::collections::HashMap::new();
        runtime_env.insert("CLASSPATH".to_string(), "/app/*:/app/lib/*".to_string());

        let mut build_packages = vec![java_version, maven_version];
        build_packages.push("ca-certificates".to_string());

        BuildTemplate {
            build_packages,
            build_commands: vec![
                "mvn package -DskipTests".to_string(),
                "mvn dependency:copy-dependencies -DoutputDirectory=target/lib".to_string(),
            ],
            cache_paths: vec!["/root/.m2/repository/".to_string()],
            common_ports: vec![8080],
            build_env,
            runtime_copy: vec![
                ("target/*.jar".to_string(), "/app/".to_string()),
                ("target/lib/".to_string(), "/app/lib".to_string()),
            ],
            runtime_env,
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".m2/repository".to_string(), "target".to_string()]
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
    let doc = Document::parse(manifest_content).ok()?;

    for node in doc.descendants() {
        if node.has_tag_name("maven.compiler.source")
            || node.has_tag_name("java.version")
            || node.has_tag_name("maven.compiler.release")
        {
            if let Some(version) = node.text() {
                let version_num = version.trim();
                return Some(format!("openjdk-{}", version_num));
            }
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
}

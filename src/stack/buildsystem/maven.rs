//! Maven build system (Java/Kotlin)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
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
                    detections.push(DetectionStack::new(
                        BuildSystemId::Maven,
                        LanguageId::Java,
                        rel_path.clone(),
                    ));
                }
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
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

        let java_home = format!(
            "/usr/lib/jvm/java-{}-openjdk",
            java_version.trim_start_matches("openjdk-")
        );

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("JAVA_HOME".to_string(), java_home);
        build_env.insert(
            "MAVEN_OPTS".to_string(),
            "-Dmaven.repo.local=/root/.m2/repository".to_string(),
        );

        let mut runtime_env = std::collections::HashMap::new();
        runtime_env.insert("CLASSPATH".to_string(), "/app/*:/app/lib/*".to_string());

        BuildTemplate {
            build_packages: vec![java_version, maven_version],
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
        if node.has_tag_name("maven.compiler.source") || node.has_tag_name("java.version") {
            if let Some(version) = node.text() {
                let version_num = version.trim();
                return Some(format!("openjdk-{}", version_num));
            }
        }
    }

    None
}

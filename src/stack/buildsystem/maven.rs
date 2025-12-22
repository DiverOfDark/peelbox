//! Maven build system (Java/Kotlin)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use anyhow::Result;
use roxmltree::Document;

pub struct MavenBuildSystem;

impl BuildSystem for MavenBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Maven
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "pom.xml".to_string(),
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "pom.xml" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("<project") || content.contains("<artifactId>")
        } else {
            true
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "maven:3.9-eclipse-temurin-21".to_string(),
            runtime_image: "eclipse-temurin:21-jre".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec!["mvn clean package -DskipTests".to_string()],
            cache_paths: vec!["/root/.m2/repository/".to_string()],
            artifacts: vec!["target/*.jar".to_string()],
            common_ports: vec![8080],
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

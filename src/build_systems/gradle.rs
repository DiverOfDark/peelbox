//! Gradle build system (Java/Kotlin)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct GradleBuildSystem;

impl BuildSystem for GradleBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Gradle
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "build.gradle",
                priority: 10,
            },
            ManifestPattern {
                filename: "build.gradle.kts",
                priority: 10,
            },
            ManifestPattern {
                filename: "settings.gradle",
                priority: 5,
            },
            ManifestPattern {
                filename: "settings.gradle.kts",
                priority: 5,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "build.gradle" | "build.gradle.kts" => {
                if let Some(content) = manifest_content {
                    content.contains("plugins") || content.contains("dependencies")
                } else {
                    true
                }
            }
            "settings.gradle" | "settings.gradle.kts" => true,
            _ => false,
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "gradle:8.5-jdk21".to_string(),
            runtime_image: "eclipse-temurin:21-jre".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec!["gradle build -x test".to_string()],
            cache_paths: vec![
                "/root/.gradle/caches/".to_string(),
                "/root/.gradle/wrapper/".to_string(),
            ],
            artifacts: vec!["build/libs/*.jar".to_string()],
            common_ports: vec![8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".gradle".to_string(), "build".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("include(") || content.contains("include '")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> &[&str] {
        &["settings.gradle", "settings.gradle.kts"]
    }
}

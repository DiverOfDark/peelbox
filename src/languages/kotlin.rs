//! Kotlin language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

pub struct KotlinLanguage;

impl LanguageDefinition for KotlinLanguage {
    fn name(&self) -> &str {
        "Kotlin"
    }

    fn extensions(&self) -> &[&str] {
        &["kt", "kts"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "build.gradle.kts",
                build_system: "gradle",
                priority: 8,
            },
            ManifestPattern {
                filename: "build.gradle",
                build_system: "gradle",
                priority: 5,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        match manifest_name {
            "build.gradle.kts" => {
                let mut confidence = 0.85;
                if let Some(content) = manifest_content {
                    if content.contains("kotlin(") || content.contains("org.jetbrains.kotlin") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: "gradle".to_string(),
                    confidence,
                })
            }
            "build.gradle" => {
                if let Some(content) = manifest_content {
                    if content.contains("kotlin") || content.contains("org.jetbrains.kotlin") {
                        return Some(DetectionResult {
                            build_system: "gradle".to_string(),
                            confidence: 0.9,
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "gradle" {
            return None;
        }

        Some(BuildTemplate {
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
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["gradle"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = KotlinLanguage;
        assert_eq!(lang.name(), "Kotlin");
    }

    #[test]
    fn test_extensions() {
        let lang = KotlinLanguage;
        assert!(lang.extensions().contains(&"kt"));
        assert!(lang.extensions().contains(&"kts"));
    }

    #[test]
    fn test_detect_gradle_kts() {
        let lang = KotlinLanguage;
        let result = lang.detect("build.gradle.kts", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "gradle");
    }

    #[test]
    fn test_detect_kotlin_plugin() {
        let lang = KotlinLanguage;
        let content = r#"
plugins {
    kotlin("jvm") version "1.9.0"
}
"#;
        let result = lang.detect("build.gradle.kts", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_gradle_without_kotlin() {
        let lang = KotlinLanguage;
        let content = "plugins { java }";
        let result = lang.detect("build.gradle", Some(content));
        assert!(result.is_none());
    }

    #[test]
    fn test_build_template() {
        let lang = KotlinLanguage;
        let template = lang.build_template("gradle");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("gradle"));
    }
}

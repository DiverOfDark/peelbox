//! Java language definition (Maven and Gradle)

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

pub struct JavaLanguage;

impl LanguageDefinition for JavaLanguage {
    fn name(&self) -> &str {
        "Java"
    }

    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "pom.xml",
                build_system: "maven",
                priority: 10,
            },
            ManifestPattern {
                filename: "build.gradle",
                build_system: "gradle",
                priority: 10,
            },
            ManifestPattern {
                filename: "build.gradle.kts",
                build_system: "gradle",
                priority: 10,
            },
            ManifestPattern {
                filename: "settings.gradle",
                build_system: "gradle",
                priority: 5,
            },
            ManifestPattern {
                filename: "settings.gradle.kts",
                build_system: "gradle",
                priority: 5,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        match manifest_name {
            "pom.xml" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("<project") || content.contains("<artifactId>") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: "maven".to_string(),
                    confidence,
                })
            }
            "build.gradle" | "build.gradle.kts" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("plugins") || content.contains("dependencies") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: "gradle".to_string(),
                    confidence,
                })
            }
            "settings.gradle" | "settings.gradle.kts" => Some(DetectionResult {
                build_system: "gradle".to_string(),
                confidence: 0.7,
            }),
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        match build_system {
            "maven" => Some(BuildTemplate {
                build_image: "maven:3.9-eclipse-temurin-21".to_string(),
                runtime_image: "eclipse-temurin:21-jre".to_string(),
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec!["mvn clean package -DskipTests".to_string()],
                cache_paths: vec!["/root/.m2/repository/".to_string()],
                artifacts: vec!["target/*.jar".to_string()],
                common_ports: vec![8080],
            }),
            "gradle" => Some(BuildTemplate {
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
            }),
            _ => None,
        }
    }

    fn build_systems(&self) -> &[&str] {
        &["maven", "gradle"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = JavaLanguage;
        assert_eq!(lang.name(), "Java");
    }

    #[test]
    fn test_extensions() {
        let lang = JavaLanguage;
        assert_eq!(lang.extensions(), &["java"]);
    }

    #[test]
    fn test_manifest_files() {
        let lang = JavaLanguage;
        let manifests = lang.manifest_files();
        assert!(manifests.iter().any(|m| m.filename == "pom.xml"));
        assert!(manifests.iter().any(|m| m.filename == "build.gradle"));
        assert!(manifests.iter().any(|m| m.filename == "build.gradle.kts"));
    }

    #[test]
    fn test_detect_maven() {
        let lang = JavaLanguage;
        let result = lang.detect("pom.xml", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "maven");
    }

    #[test]
    fn test_detect_maven_with_content() {
        let lang = JavaLanguage;
        let content = r#"<project><artifactId>myapp</artifactId></project>"#;
        let result = lang.detect("pom.xml", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_gradle() {
        let lang = JavaLanguage;
        let result = lang.detect("build.gradle", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "gradle");
    }

    #[test]
    fn test_detect_gradle_kts() {
        let lang = JavaLanguage;
        let result = lang.detect("build.gradle.kts", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "gradle");
    }

    #[test]
    fn test_build_template_maven() {
        let lang = JavaLanguage;
        let template = lang.build_template("maven");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("maven"));
        assert!(t.build_commands.iter().any(|c| c.contains("mvn")));
    }

    #[test]
    fn test_build_template_gradle() {
        let lang = JavaLanguage;
        let template = lang.build_template("gradle");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("gradle"));
        assert!(t.build_commands.iter().any(|c| c.contains("gradle")));
    }
}

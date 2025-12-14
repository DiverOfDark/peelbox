//! Java/Kotlin language definition (Maven and Gradle)

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
use regex::Regex;

pub struct JavaLanguage;

impl LanguageDefinition for JavaLanguage {
    fn name(&self) -> &str {
        "Java"
    }

    fn extensions(&self) -> &[&str] {
        &["java", "kt", "kts"]
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
            ManifestPattern {
                filename: ".java-version",
                build_system: "maven",
                priority: 3,
            },
        ]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
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
            ".java-version" => Some(DetectionResult {
                build_system: "maven".to_string(),
                confidence: 0.5,
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

    fn excluded_dirs(&self) -> &[&str] {
        &["target", "build", ".gradle", ".m2"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &["settings.gradle", "settings.gradle.kts"]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // Check pom.xml patterns
        if content.contains("<project") {
            // <maven.compiler.source>17</maven.compiler.source>
            if let Some(caps) =
                Regex::new(r"<maven\.compiler\.source>(\d+)</maven\.compiler\.source>")
                    .ok()
                    .and_then(|re| re.captures(content))
            {
                return Some(caps.get(1)?.as_str().to_string());
            }
            // <java.version>17</java.version>
            if let Some(caps) = Regex::new(r"<java\.version>(\d+)</java\.version>")
                .ok()
                .and_then(|re| re.captures(content))
            {
                return Some(caps.get(1)?.as_str().to_string());
            }
            // <release>17</release>
            if let Some(caps) = Regex::new(r"<release>(\d+)</release>")
                .ok()
                .and_then(|re| re.captures(content))
            {
                return Some(caps.get(1)?.as_str().to_string());
            }
        }

        // Check build.gradle(.kts) patterns
        // sourceCompatibility = JavaVersion.VERSION_17 or "17"
        if let Some(caps) =
            Regex::new(r#"sourceCompatibility\s*=\s*(?:JavaVersion\.VERSION_)?["']?(\d+)"#)
                .ok()
                .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // java { toolchain { languageVersion.set(JavaLanguageVersion.of(17)) } }
        if let Some(caps) = Regex::new(r"languageVersion\.set\(JavaLanguageVersion\.of\((\d+)\)\)")
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // .java-version file (just contains the version number)
        if !content.contains('<') && !content.contains('{') {
            let trimmed = content.trim();
            if Regex::new(r"^\d+(\.\d+)?$").ok()?.is_match(trimmed) {
                return Some(trimmed.to_string());
            }
        }

        None
    }

    fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "pom.xml" => {
                if let Some(content) = manifest_content {
                    content.contains("<modules>") || content.contains("<module>")
                } else {
                    false
                }
            }
            "settings.gradle" | "settings.gradle.kts" => {
                if let Some(content) = manifest_content {
                    content.contains("include(") || content.contains("include ")
                } else {
                    false
                }
            }
            _ => false,
        }
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
        assert!(lang.extensions().contains(&"java"));
        assert!(lang.extensions().contains(&"kt"));
        assert!(lang.extensions().contains(&"kts"));
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

    #[test]
    fn test_excluded_dirs() {
        let lang = JavaLanguage;
        assert!(lang.excluded_dirs().contains(&"target"));
        assert!(lang.excluded_dirs().contains(&".gradle"));
    }

    #[test]
    fn test_workspace_configs() {
        let lang = JavaLanguage;
        assert!(lang.workspace_configs().contains(&"settings.gradle"));
    }

    #[test]
    fn test_detect_version_pom_maven_compiler() {
        let lang = JavaLanguage;
        let content = r#"<project><properties><maven.compiler.source>17</maven.compiler.source></properties></project>"#;
        assert_eq!(lang.detect_version(Some(content)), Some("17".to_string()));
    }

    #[test]
    fn test_detect_version_pom_java_version() {
        let lang = JavaLanguage;
        let content =
            r#"<project><properties><java.version>21</java.version></properties></project>"#;
        assert_eq!(lang.detect_version(Some(content)), Some("21".to_string()));
    }

    #[test]
    fn test_detect_version_gradle_source_compat() {
        let lang = JavaLanguage;
        let content = r#"sourceCompatibility = "17""#;
        assert_eq!(lang.detect_version(Some(content)), Some("17".to_string()));
    }

    #[test]
    fn test_detect_version_gradle_toolchain() {
        let lang = JavaLanguage;
        let content = r#"java { toolchain { languageVersion.set(JavaLanguageVersion.of(21)) } }"#;
        assert_eq!(lang.detect_version(Some(content)), Some("21".to_string()));
    }

    #[test]
    fn test_detect_version_java_version_file() {
        let lang = JavaLanguage;
        let content = "17";
        assert_eq!(lang.detect_version(Some(content)), Some("17".to_string()));
    }

    #[test]
    fn test_is_workspace_root_maven_modules() {
        let lang = JavaLanguage;
        let content = r#"
<project>
    <modules>
        <module>module-a</module>
        <module>module-b</module>
    </modules>
</project>
"#;
        assert!(lang.is_workspace_root("pom.xml", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_maven_no_modules() {
        let lang = JavaLanguage;
        let content = r#"
<project>
    <artifactId>my-app</artifactId>
</project>
"#;
        assert!(!lang.is_workspace_root("pom.xml", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_gradle_settings() {
        let lang = JavaLanguage;
        let content = r#"
rootProject.name = "my-project"
include("module-a")
include("module-b")
"#;
        assert!(lang.is_workspace_root("settings.gradle", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_gradle_settings_kts() {
        let lang = JavaLanguage;
        let content = r#"
rootProject.name = "my-project"
include("module-a", "module-b")
"#;
        assert!(lang.is_workspace_root("settings.gradle.kts", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_gradle_no_includes() {
        let lang = JavaLanguage;
        let content = r#"
rootProject.name = "single-project"
"#;
        assert!(!lang.is_workspace_root("settings.gradle", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_wrong_file() {
        let lang = JavaLanguage;
        assert!(!lang.is_workspace_root("build.gradle", Some("<modules></modules>")));
    }
}

//! Java/Kotlin language definition (Maven and Gradle)

use super::{
    Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition,
};
use regex::Regex;
use std::collections::HashSet;

pub struct JavaLanguage;

impl LanguageDefinition for JavaLanguage {
    fn name(&self) -> &str {
        "Java"
    }

    fn extensions(&self) -> &[&str] {
        &["java", "kt", "kts"]
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

    fn compatible_build_systems(&self) -> &[&str] {
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

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        if manifest_content.contains("<project") {
            self.parse_maven_dependencies(manifest_content)
        } else if manifest_content.contains("dependencies")
            || manifest_content.contains("implementation")
        {
            self.parse_gradle_dependencies(manifest_content)
        } else {
            DependencyInfo::empty()
        }
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"System\.getenv\("([A-Z_][A-Z0-9_]*)""#, "System.getenv")]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r#"server\.port\s*=\s*(\d{4,5})"#, "application.properties"),
            (r#"port:\s*(\d{4,5})"#, "application.yml"),
        ]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"@GetMapping\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "Spring")]
    }

    fn default_health_endpoints(&self) -> Vec<(&'static str, &'static str)> {
        vec![("/actuator/health", "Spring Boot")]
    }

    fn is_main_file(&self, fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name.ends_with("Application.java") || file_name.ends_with("Application.kt") {
                if let Some(path_str) = file_path.to_str() {
                    if path_str.contains("src/main/java/") || path_str.contains("src/main/kotlin/")
                    {
                        return true;
                    }
                }
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("public static void main") {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<&'static str> {
        Some("java")
    }

    fn default_port(&self) -> Option<u16> {
        Some(8080)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("java -jar app.jar".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }
}

impl JavaLanguage {
    fn parse_maven_dependencies(&self, content: &str) -> DependencyInfo {
        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        let dep_re = Regex::new(r"<dependency>\s*<groupId>([^<]+)</groupId>\s*<artifactId>([^<]+)</artifactId>(?:\s*<version>([^<]+)</version>)?").ok();

        if let Some(ref re) = dep_re {
            for caps in re.captures_iter(content) {
                if let (Some(group), Some(artifact)) = (caps.get(1), caps.get(2)) {
                    let name = format!("{}:{}", group.as_str(), artifact.as_str());
                    if seen.contains(&name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let version = caps.get(3).map(|v| v.as_str().to_string());

                    external_deps.push(Dependency {
                        name,
                        version,
                        is_internal: false,
                    });
                }
            }
        }

        let module_re = Regex::new(r"<module>([^<]+)</module>").ok();
        if let Some(ref re) = module_re {
            for caps in re.captures_iter(content) {
                if let Some(module_name) = caps.get(1) {
                    let name = module_name.as_str().to_string();
                    if !seen.contains(&name) {
                        internal_deps.push(Dependency {
                            name: name.clone(),
                            version: Some("module".to_string()),
                            is_internal: true,
                        });
                        seen.insert(name);
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn parse_gradle_dependencies(&self, content: &str) -> DependencyInfo {
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        let dep_re = Regex::new(r#"(?:implementation|api|compileOnly|runtimeOnly|testImplementation)\s*[("']+([^:"']+):([^:"']+):?([^"')]*)"#).ok();

        if let Some(ref re) = dep_re {
            for caps in re.captures_iter(content) {
                if let (Some(group), Some(artifact)) = (caps.get(1), caps.get(2)) {
                    let name = format!("{}:{}", group.as_str(), artifact.as_str());
                    if seen.contains(&name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let version = caps.get(3).and_then(|v| {
                        let s = v.as_str().trim();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s.to_string())
                        }
                    });

                    external_deps.push(Dependency {
                        name,
                        version,
                        is_internal: false,
                    });
                }
            }
        }

        DependencyInfo {
            internal_deps: vec![],
            external_deps,
            detected_by: DetectionMethod::Deterministic,
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
    fn test_compatible_build_systems() {
        let lang = JavaLanguage;
        let systems = lang.compatible_build_systems();
        assert!(systems.contains(&"maven"));
        assert!(systems.contains(&"gradle"));
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

    #[test]
    fn test_parse_dependencies_maven() {
        let lang = JavaLanguage;
        let content = r#"
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
            <version>3.2.0</version>
        </dependency>
        <dependency>
            <groupId>com.h2database</groupId>
            <artifactId>h2</artifactId>
        </dependency>
    </dependencies>
</project>
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("spring-boot-starter-web")));
        assert!(deps.external_deps.iter().any(|d| d.name.contains("h2")));
    }

    #[test]
    fn test_parse_dependencies_maven_modules() {
        let lang = JavaLanguage;
        let content = r#"
<project>
    <modules>
        <module>module-a</module>
        <module>module-b</module>
    </modules>
</project>
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.internal_deps.len(), 2);
        assert!(deps.internal_deps.iter().any(|d| d.name == "module-a"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "module-b"));
    }

    #[test]
    fn test_parse_dependencies_gradle() {
        let lang = JavaLanguage;
        let content = r#"
dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web:3.2.0")
    implementation 'com.fasterxml.jackson.core:jackson-databind:2.15.0'
    testImplementation("org.junit.jupiter:junit-jupiter")
}
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 3);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("spring-boot-starter-web")));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("jackson-databind")));
    }

    #[test]
    fn test_parse_dependencies_gradle_kotlin_dsl() {
        let lang = JavaLanguage;
        let content = r#"
dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web:3.2.0")
    api("com.google.guava:guava:32.1.0-jre")
    compileOnly("org.projectlombok:lombok:1.18.30")
    runtimeOnly("com.h2database:h2:2.2.224")
    testImplementation("org.junit.jupiter:junit-jupiter")
}
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 5);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("spring-boot-starter-web")));
        assert!(deps.external_deps.iter().any(|d| d.name.contains("guava")));
    }

    #[test]
    fn test_parse_dependencies_gradle_groovy_dsl() {
        let lang = JavaLanguage;
        let content = r#"
dependencies {
    implementation 'org.springframework.boot:spring-boot-starter-web:3.2.0'
    api 'com.google.guava:guava:32.1.0-jre'
    compileOnly 'org.projectlombok:lombok:1.18.30'
    runtimeOnly 'com.h2database:h2:2.2.224'
    testImplementation 'org.junit.jupiter:junit-jupiter:5.10.0'
}
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 5);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("spring-boot-starter-web")
                && d.version == Some("3.2.0".to_string())));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("junit-jupiter")));
    }

    #[test]
    fn test_parse_dependencies_gradle_mixed_syntax() {
        let lang = JavaLanguage;
        let content = r#"
dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web:3.2.0")
    implementation 'com.fasterxml.jackson.core:jackson-databind:2.15.0'
    api("com.google.guava:guava:32.1.0-jre")
    testImplementation "org.junit.jupiter:junit-jupiter:5.10.0"
}
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 4);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("spring-boot-starter-web")));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name.contains("jackson-databind")));
    }
}

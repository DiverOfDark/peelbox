//! C/C++ language definition

use super::{Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition};
use regex::Regex;

pub struct CppLanguage;

impl LanguageDefinition for CppLanguage {
    fn id(&self) -> crate::stack::LanguageId {
        crate::stack::LanguageId::Cpp
    }

    fn extensions(&self) -> Vec<String> {
        vec![
            "cpp".to_string(),
            "cc".to_string(),
            "cxx".to_string(),
            "c".to_string(),
            "h".to_string(),
            "hpp".to_string(),
            "hxx".to_string(),
        ]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "CMakeLists.txt" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("cmake_minimum_required") || content.contains("project(") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: crate::stack::BuildSystemId::CMake,
                    confidence,
                })
            }
            "Makefile" => Some(DetectionResult {
                build_system: crate::stack::BuildSystemId::Make,
                confidence: 0.85,
            }),
            "meson.build" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("project(") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: crate::stack::BuildSystemId::Meson,
                    confidence,
                })
            }
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["cmake".to_string(), "make".to_string(), "meson".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec![
            "build".to_string(),
            "builddir".to_string(),
            "cmake-build-debug".to_string(),
            "cmake-build-release".to_string(),
        ]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let mut external_deps = Vec::new();

        if let Ok(re) = Regex::new(r"find_package\s*\(\s*(\w+)") {
            for cap in re.captures_iter(manifest_content) {
                if let Some(name) = cap.get(1) {
                    external_deps.push(Dependency {
                        name: name.as_str().to_string(),
                        version: None,
                        is_internal: false,
                    });
                }
            }
        }

        if let Ok(re) = Regex::new(r"target_link_libraries\s*\([^)]*\s+(\w+)") {
            for cap in re.captures_iter(manifest_content) {
                if let Some(name) = cap.get(1) {
                    let lib_name = name.as_str().to_string();
                    if !external_deps.iter().any(|d| d.name == lib_name) {
                        external_deps.push(Dependency {
                            name: lib_name,
                            version: None,
                            is_internal: false,
                        });
                    }
                }
            }
        }

        if manifest_content.contains("[requires]") {
            if let Some(requires_section) = manifest_content.split("[requires]").nth(1) {
                let section_end = requires_section.find('[').unwrap_or(requires_section.len());
                let section = &requires_section[..section_end];

                for line in section.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        if let Some(pkg) = trimmed.split('/').next() {
                            external_deps.push(Dependency {
                                name: pkg.to_string(),
                                version: None,
                                is_internal: false,
                            });
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps: vec![],
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"getenv\("([A-Z_][A-Z0-9_]*)"\)"#.to_string(),
            "getenv".to_string(),
        )]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"bind\([^,)]*,\s*(\d{4,5})"#.to_string(),
            "bind()".to_string(),
        )]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"CROW_ROUTE.*\(([/\w\-]*health[/\w\-]*)\)"#.to_string(),
            "Crow/Beast".to_string(),
        )]
    }

    fn default_health_endpoints(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<String> {
        vec![]
    }

    fn is_main_file(&self, fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name == "main.cpp" || file_name == "main.cc" || file_name == "main.cxx" {
                return true;
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("int main(") {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<String> {
        Some("c++".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(8080)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("./app".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = CppLanguage;
        assert!(lang.extensions().iter().any(|s| s == "cpp"));
        assert!(lang.extensions().iter().any(|s| s == "c"));
        assert!(lang.extensions().iter().any(|s| s == "hpp"));
    }

    #[test]
    fn test_detect_cmake() {
        let lang = CppLanguage;
        let result = lang.detect("CMakeLists.txt", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::CMake);
    }

    #[test]
    fn test_detect_cmake_with_content() {
        let lang = CppLanguage;
        let content = "cmake_minimum_required(VERSION 3.16)\nproject(MyApp)";
        let result = lang.detect("CMakeLists.txt", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_makefile() {
        let lang = CppLanguage;
        let result = lang.detect("Makefile", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Make);
    }

    #[test]
    fn test_detect_meson() {
        let lang = CppLanguage;
        let result = lang.detect("meson.build", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Meson);
    }

    #[test]
    fn test_compatible_build_systems() {
        let lang = CppLanguage;
        assert_eq!(lang.compatible_build_systems(), &["cmake", "make", "meson"]);
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = CppLanguage;
        assert!(lang.excluded_dirs().iter().any(|s| s == "build"));
        assert!(lang
            .excluded_dirs()
            .iter()
            .any(|s| s == "cmake-build-debug"));
    }

    #[test]
    fn test_parse_dependencies_cmake() {
        let lang = CppLanguage;
        let content = r#"
cmake_minimum_required(VERSION 3.16)
project(MyApp)

find_package(Boost REQUIRED)
find_package(OpenSSL REQUIRED)
target_link_libraries(myapp Boost::system pthread)
"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.iter().any(|d| d.name == "Boost"));
        assert!(deps.external_deps.iter().any(|d| d.name == "OpenSSL"));
    }

    #[test]
    fn test_parse_dependencies_conan() {
        let lang = CppLanguage;
        let content = r#"
[requires]
boost/1.81.0
openssl/3.0.0

[generators]
cmake
"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.iter().any(|d| d.name == "boost"));
        assert!(deps.external_deps.iter().any(|d| d.name == "openssl"));
    }

    #[test]
    fn test_parse_dependencies_empty() {
        let lang = CppLanguage;
        let content = "project(MyApp)";
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.is_empty());
    }
}

//! C/C++ language definition

use super::{
    BuildTemplate, Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition, ManifestPattern,
};
use regex::Regex;

pub struct CppLanguage;

impl LanguageDefinition for CppLanguage {
    fn name(&self) -> &str {
        "C++"
    }

    fn extensions(&self) -> &[&str] {
        &["cpp", "cc", "cxx", "c", "h", "hpp", "hxx"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "CMakeLists.txt",
                build_system: "cmake",
                priority: 10,
            },
            ManifestPattern {
                filename: "Makefile",
                build_system: "make",
                priority: 8,
            },
            ManifestPattern {
                filename: "meson.build",
                build_system: "meson",
                priority: 10,
            },
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
                    build_system: "cmake".to_string(),
                    confidence,
                })
            }
            "Makefile" => Some(DetectionResult {
                build_system: "make".to_string(),
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
                    build_system: "meson".to_string(),
                    confidence,
                })
            }
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        match build_system {
            "cmake" => Some(BuildTemplate {
                build_image: "gcc:13".to_string(),
                runtime_image: "debian:bookworm-slim".to_string(),
                build_packages: vec!["cmake".to_string(), "make".to_string()],
                runtime_packages: vec!["libstdc++6".to_string()],
                build_commands: vec![
                    "cmake -B build -DCMAKE_BUILD_TYPE=Release".to_string(),
                    "cmake --build build --config Release".to_string(),
                ],
                cache_paths: vec!["build/".to_string()],
                artifacts: vec!["build/{project_name}".to_string()],
                common_ports: vec![8080],
            }),
            "make" => Some(BuildTemplate {
                build_image: "gcc:13".to_string(),
                runtime_image: "debian:bookworm-slim".to_string(),
                build_packages: vec!["make".to_string()],
                runtime_packages: vec!["libstdc++6".to_string()],
                build_commands: vec!["make".to_string()],
                cache_paths: vec![],
                artifacts: vec!["{project_name}".to_string()],
                common_ports: vec![8080],
            }),
            "meson" => Some(BuildTemplate {
                build_image: "gcc:13".to_string(),
                runtime_image: "debian:bookworm-slim".to_string(),
                build_packages: vec!["meson".to_string(), "ninja-build".to_string()],
                runtime_packages: vec!["libstdc++6".to_string()],
                build_commands: vec![
                    "meson setup builddir --buildtype=release".to_string(),
                    "meson compile -C builddir".to_string(),
                ],
                cache_paths: vec!["builddir/".to_string()],
                artifacts: vec!["builddir/{project_name}".to_string()],
                common_ports: vec![8080],
            }),
            _ => None,
        }
    }

    fn build_systems(&self) -> &[&str] {
        &["cmake", "make", "meson"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &[
            "build",
            "builddir",
            "cmake-build-debug",
            "cmake-build-release",
        ]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
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

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"getenv\("([A-Z_][A-Z0-9_]*)"\)"#, "getenv")]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"bind\([^,)]*,\s*(\d{4,5})"#, "bind()")]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn default_health_endpoints(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<&'static str> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = CppLanguage;
        assert_eq!(lang.name(), "C++");
    }

    #[test]
    fn test_extensions() {
        let lang = CppLanguage;
        assert!(lang.extensions().contains(&"cpp"));
        assert!(lang.extensions().contains(&"c"));
        assert!(lang.extensions().contains(&"hpp"));
    }

    #[test]
    fn test_detect_cmake() {
        let lang = CppLanguage;
        let result = lang.detect("CMakeLists.txt", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "cmake");
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
        assert_eq!(r.build_system, "make");
    }

    #[test]
    fn test_detect_meson() {
        let lang = CppLanguage;
        let result = lang.detect("meson.build", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "meson");
    }

    #[test]
    fn test_build_template_cmake() {
        let lang = CppLanguage;
        let template = lang.build_template("cmake");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("cmake")));
    }

    #[test]
    fn test_build_template_make() {
        let lang = CppLanguage;
        let template = lang.build_template("make");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("make")));
    }

    #[test]
    fn test_build_template_meson() {
        let lang = CppLanguage;
        let template = lang.build_template("meson");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("meson")));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = CppLanguage;
        assert!(lang.excluded_dirs().contains(&"build"));
        assert!(lang.excluded_dirs().contains(&"cmake-build-debug"));
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

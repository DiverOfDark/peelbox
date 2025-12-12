//! C/C++ language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

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
}

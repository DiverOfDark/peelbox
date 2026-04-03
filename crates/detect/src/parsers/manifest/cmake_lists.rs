use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const C: LanguageId = LanguageId::new("c");
const CPP: LanguageId = LanguageId::new("c++");
const CMAKE: BuildSystemId = BuildSystemId::new("cmake");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "c++", display_name: "C++", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "cmake", display_name: "CMake", aliases: &["cmake"] }
}

pub struct CMakeListsParser;

impl ManifestParser for CMakeListsParser {
    fn filenames(&self) -> &[&str] {
        &["CMakeLists.txt"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("cmake_minimum_required") && !content.contains("project(") {
            return None;
        }

        let name_re = regex::Regex::new(r"project\s*\(\s*([\w-]+)").ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        // Detect language: check for C-only indicators
        // CMake project() can specify LANGUAGES: project(name LANGUAGES C) or project(name LANGUAGES CXX)
        // Also check for CMAKE_C_STANDARD vs CMAKE_CXX_STANDARD and .c vs .cpp source files
        let is_c_only = detect_c_only_project(content);
        let language = if is_c_only { C } else { CPP };

        // Runtime packages: C++ needs libstdc++, pure C does not
        let mut runtime_packages = vec!["glibc".into(), "ca-certificates".into()];
        if !is_c_only {
            runtime_packages.push("libstdc++".into());
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: CMAKE,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "build-base".into(),
                    "cmake".into(),
                    "gcc".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "cmake -B build -DCMAKE_BUILD_TYPE=Release".into(),
                    "cmake --build build --config Release".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["CMakeCache.txt".into(), "build".into()],
                artifacts: vec![(format!("build/{}", name), format!("/app/{}", name))],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Detects whether a CMake project is C-only (no C++ involved).
///
/// Returns true when:
/// - `project()` explicitly declares `LANGUAGES C` without `CXX`, or
/// - Only `.c`/`.h` source files are referenced (no `.cpp`/`.cxx`/`.cc`), and
///   `CMAKE_CXX_STANDARD` is absent
fn detect_c_only_project(content: &str) -> bool {
    // Check for explicit LANGUAGES declaration in project()
    if let Ok(lang_re) = regex::Regex::new(r"(?i)project\s*\([^)]*LANGUAGES\s+([^)]+)\)") {
        if let Some(caps) = lang_re.captures(content) {
            let languages = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let has_c = languages.split_whitespace().any(|l| l == "C");
            let has_cxx = languages
                .split_whitespace()
                .any(|l| l == "CXX" || l == "C++");
            if has_c && !has_cxx {
                return true;
            }
            if has_cxx {
                return false;
            }
        }
    }

    // Heuristic: presence of CMAKE_CXX_STANDARD or .cpp/.cxx/.cc source references means C++
    let has_cxx_standard = content.contains("CMAKE_CXX_STANDARD");
    let has_cpp_sources = content.contains(".cpp")
        || content.contains(".cxx")
        || content.contains(".cc")
        || content.contains(".hpp");

    if has_cxx_standard || has_cpp_sources {
        return false;
    }

    // If CMAKE_C_STANDARD is present and no C++ indicators, it's C
    let has_c_standard = content.contains("CMAKE_C_STANDARD");
    let has_c_sources = content.contains(".c");

    has_c_standard || has_c_sources
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(CMakeListsParser))
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["C"],
        extensions: &["c", "h"],
        port_patterns: &[
            r#"htons\(\s*(\d{4,5})\s*\)"#,
            r#"port\s*=\s*(\d{4,5})"#,
        ],
        health_patterns: &[
            r#"==\s*"([/\w\-]*health[/\w\-]*)""#,
            r#"strcmp\([^,]*,\s*"([/\w\-]*health[/\w\-]*)""#,
        ],
        env_var_patterns: &[r#"getenv\(\s*["']([A-Z_][A-Z0-9_]*)["']"#],
    }
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["C++"],
        extensions: &["cpp", "cxx", "cc", "hpp", "h"],
        port_patterns: &[
            r#"htons\(\s*(\d{4,5})\s*\)"#,
            r#"port\s*=\s*(\d{4,5})"#,
        ],
        health_patterns: &[
            r#"==\s*"([/\w\-]*health[/\w\-]*)""#,
            r#"strcmp\([^,]*,\s*"([/\w\-]*health[/\w\-]*)""#,
        ],
        env_var_patterns: &[r#"getenv\(\s*["']([A-Z_][A-Z0-9_]*)["']"#],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;
    use std::path::Path;

    #[test]
    fn test_cmake_cpp_project() {
        let parser = CMakeListsParser;
        let content = r#"cmake_minimum_required(VERSION 3.15)
project(app VERSION 1.0.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_executable(app src/main.cpp)
"#;
        let manifest = parser.parse(Path::new("CMakeLists.txt"), content).unwrap();
        assert_eq!(manifest.language, CPP);
        assert_eq!(manifest.build_system, CMAKE);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(manifest
            .runtime_config
            .packages
            .contains(&"libstdc++".into()));
    }

    #[test]
    fn test_cmake_c_only_project() {
        let parser = CMakeListsParser;
        let content = r#"cmake_minimum_required(VERSION 3.10)
project(myserver LANGUAGES C)

set(CMAKE_C_STANDARD 11)

add_executable(myserver src/main.c)
"#;
        let manifest = parser.parse(Path::new("CMakeLists.txt"), content).unwrap();
        assert_eq!(manifest.language, C);
        assert_eq!(manifest.build_system, CMAKE);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "myserver");
        // Pure C should not have libstdc++
        assert!(!manifest
            .runtime_config
            .packages
            .contains(&"libstdc++".into()));
    }

    #[test]
    fn test_cmake_mixed_c_cxx_project() {
        let parser = CMakeListsParser;
        let content = r#"cmake_minimum_required(VERSION 3.15)
project(myapp LANGUAGES C CXX)

add_executable(myapp src/main.cpp src/util.c)
"#;
        let manifest = parser.parse(Path::new("CMakeLists.txt"), content).unwrap();
        assert_eq!(manifest.language, CPP); // Mixed -> C++
        assert!(manifest
            .runtime_config
            .packages
            .contains(&"libstdc++".into()));
    }

    #[test]
    fn test_cmake_rejects_no_project() {
        let parser = CMakeListsParser;
        let content = "# This is just a comment";
        assert!(parser.parse(Path::new("CMakeLists.txt"), content).is_none());
    }

    #[test]
    fn test_cmake_c_heuristic_from_sources() {
        let parser = CMakeListsParser;
        let content = r#"cmake_minimum_required(VERSION 3.10)
project(hello)

set(CMAKE_C_STANDARD 99)

add_executable(hello src/main.c)
"#;
        let manifest = parser.parse(Path::new("CMakeLists.txt"), content).unwrap();
        assert_eq!(manifest.language, C);
    }
}

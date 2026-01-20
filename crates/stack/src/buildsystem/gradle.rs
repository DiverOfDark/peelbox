//! Gradle build system (Java/Kotlin)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::language::LanguageDefinition;
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct GradleBuildSystem;

impl BuildSystem for GradleBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Gradle
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "build.gradle.kts".to_string(),
                priority: 20,
            },
            ManifestPattern {
                filename: "build.gradle".to_string(),
                priority: 15,
            },
            ManifestPattern {
                filename: "settings.gradle.kts".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "settings.gradle".to_string(),
                priority: 10,
            },
        ]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();
        let mut dir_has_build_file = std::collections::HashSet::new();

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str());

            if matches!(filename, Some("build.gradle") | Some("build.gradle.kts")) {
                let abs_path = repo_root.join(rel_path);
                let content = fs.read_to_string(&abs_path).ok();
                let has_build_content = if let Some(c) = content.as_deref() {
                    c.contains("plugins") || c.contains("dependencies")
                } else {
                    true
                };

                if has_build_content {
                    let lang = crate::language::JavaLanguage;
                    let project_dir = rel_path.parent().unwrap_or(Path::new(""));

                    if lang.is_runnable(fs, repo_root, project_dir, file_tree, content.as_deref()) {
                        if let Some(parent) = rel_path.parent() {
                            dir_has_build_file.insert(parent.to_path_buf());
                        }
                        detections.push(DetectionStack::new(
                            BuildSystemId::Gradle,
                            LanguageId::Java,
                            rel_path.clone(),
                        ));
                    }
                }
            }
        }

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str());

            if matches!(
                filename,
                Some("settings.gradle") | Some("settings.gradle.kts")
            ) {
                if let Some(parent) = rel_path.parent() {
                    if !dir_has_build_file.contains(parent) {
                        detections.push(DetectionStack::new(
                            BuildSystemId::Gradle,
                            LanguageId::Java,
                            rel_path.clone(),
                        ));
                    }
                }
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let parsed_version = manifest_content.and_then(parse_java_version);

        let java_version = if let Some(ver) = parsed_version {
            if wolfi_index.has_package(&ver) {
                ver
            } else {
                eprintln!(
                    "Warning: Requested Java version '{}' not found in Wolfi packages. Falling back to latest.",
                    ver
                );
                let latest = wolfi_index
                    .get_latest_version("openjdk")
                    .expect("Failed to get openjdk version from Wolfi index");
                latest
            }
        } else {
            let latest = wolfi_index
                .get_latest_version("openjdk")
                .expect("Failed to get openjdk version from Wolfi index");
            latest
        };

        let _runtime_version = format!("{}-jre", java_version);

        let gradle_version = wolfi_index
            .get_latest_version("gradle")
            .expect("Failed to get gradle version from Wolfi index");

        let java_home = if java_version == "openjdk-8" {
            "/usr/lib/jvm/java-1.8-openjdk".to_string()
        } else {
            format!(
                "/usr/lib/jvm/java-{}-openjdk",
                java_version.trim_start_matches("openjdk-")
            )
        };

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("JAVA_HOME".to_string(), java_home);
        build_env.insert("GRADLE_USER_HOME".to_string(), "/root/.gradle".to_string());
        build_env.insert(
            "GRADLE_OPTS".to_string(),
            "-Dorg.gradle.native=false".to_string(),
        );

        let mut build_packages = vec![java_version, gradle_version];
        build_packages.push("ca-certificates".to_string());

        BuildTemplate {
            build_packages,
            build_commands: vec!["gradle assemble --no-daemon --console=plain".to_string()],
            cache_paths: vec![
                "/root/.gradle/caches/".to_string(),
                "/root/.gradle/wrapper/".to_string(),
                "/root/.gradle/native/".to_string(),
            ],
            common_ports: vec![8080],
            build_env,
            runtime_copy: vec![("build/libs/*.jar".to_string(), "/app/app.jar".to_string())],
            runtime_env: std::collections::HashMap::new(),
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

    fn parse_package_metadata(
        &self,
        _manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        // Gradle build.gradle files don't contain project names
        // Project names are defined in settings.gradle or derived from directory names
        // Return error to trigger fallback to directory name
        Err(anyhow::anyhow!(
            "Gradle projects use directory names, not manifest metadata"
        ))
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![
            "settings.gradle".to_string(),
            "settings.gradle.kts".to_string(),
        ]
    }

    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        let mut patterns = Vec::new();

        for line in manifest_content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("include") {
                let include_str = if trimmed.contains('(') {
                    trimmed.split('(').nth(1).and_then(|s| s.split(')').next())
                } else {
                    None
                };

                if let Some(projects_str) = include_str {
                    for project in projects_str.split(',') {
                        let project = project.trim().trim_matches(|c| c == '\'' || c == '"');
                        if !project.is_empty() {
                            patterns.push(project.trim_start_matches(':').to_string());
                        }
                    }
                }
            }
        }

        Ok(patterns)
    }
}

fn parse_java_version(manifest_content: &str) -> Option<String> {
    for line in manifest_content.lines() {
        let trimmed = line.trim();

        if trimmed.contains("sourceCompatibility")
            || trimmed.contains("targetCompatibility")
            || trimmed.contains("languageVersion")
        {
            if let Some(version) = trimmed.split(['=', '(', ')', ' ']).find(|s| {
                let s = s.trim();
                !s.is_empty() && (s.chars().all(|c| c.is_ascii_digit()) || s.contains("VERSION_"))
            }) {
                let version_num = version
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .replace("JavaVersion.VERSION_", "")
                    .replace('_', ".");

                let version_final = if version_num.starts_with("1.") && version_num.len() > 2 {
                    version_num.get(2..).unwrap_or(&version_num).to_string()
                } else {
                    version_num
                };

                return Some(format!("openjdk-{}", version_final));
            }
        }
    }

    None
}

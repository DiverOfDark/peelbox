//! Gradle build system (Java/Kotlin)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct GradleBuildSystem;

impl BuildSystem for GradleBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Gradle
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "settings.gradle".to_string(),
                priority: 15,
            },
            ManifestPattern {
                filename: "settings.gradle.kts".to_string(),
                priority: 15,
            },
            ManifestPattern {
                filename: "build.gradle".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "build.gradle.kts".to_string(),
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

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str());

            let is_match = match filename {
                Some("build.gradle") | Some("build.gradle.kts") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        c.contains("plugins") || c.contains("dependencies")
                    } else {
                        true
                    }
                }
                Some("settings.gradle") | Some("settings.gradle.kts") => true,
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Gradle,
                    LanguageId::Java,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
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

    fn workspace_configs(&self) -> Vec<String> {
        vec!["settings.gradle".to_string(), "settings.gradle.kts".to_string()]
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

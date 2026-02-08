use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_stack::{BuildSystemId, LanguageId, RuntimeId};
use std::path::Path;

pub struct SettingsGradleParser;

impl ManifestParser for SettingsGradleParser {
    fn filenames(&self) -> &[&str] {
        &["settings.gradle", "settings.gradle.kts"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let mut members = Vec::new();
        let mut project_name = None;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("rootProject.name") {
                if let Some(value) = trimmed.split('=').nth(1) {
                    let name = value
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if !name.is_empty() {
                        project_name = Some(name);
                    }
                }
            }

            if trimmed.starts_with("include") {
                if let Some(projects_str) = trimmed
                    .split('(')
                    .nth(1)
                    .and_then(|s| s.split(')').next())
                {
                    for project in projects_str.split(',') {
                        let project = project.trim().trim_matches(|c| c == '\'' || c == '"');
                        if !project.is_empty() {
                            members.push(project.trim_start_matches(':').to_string());
                        }
                    }
                }
            }
        }

        if members.is_empty() && project_name.is_none() {
            return None;
        }

        let workspace = if !members.is_empty() {
            Some(Workspace {
                members,
                orchestrator: None,
            })
        } else {
            None
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Java,
            build_system: BuildSystemId::Gradle,
            runtime: RuntimeId::JVM,
            package: project_name.map(|name| Package {
                name,
                version: None,
                is_application: true,
            }),
            workspace,
            dependencies: Vec::new(),
            build: BuildSpec::default(),
            runtime_config: RuntimeSpec::default(),
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(SettingsGradleParser))
}

use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const JAVA: LanguageId = LanguageId::new("java");
const GRADLE: BuildSystemId = BuildSystemId::new("gradle");
const JVM: RuntimeId = RuntimeId::new("jvm");

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
                // Handle both `include('a', 'b')` and `include 'a', 'b'` syntax
                let projects_str = if let Some(inner) =
                    trimmed.split('(').nth(1).and_then(|s| s.split(')').next())
                {
                    inner.to_string()
                } else {
                    // Groovy `include 'a', 'b'` syntax (no parens)
                    trimmed.strip_prefix("include").unwrap_or("").to_string()
                };
                for project in projects_str.split(',') {
                    let project = project
                        .trim()
                        .trim_matches(|c: char| c == '\'' || c == '"' || c.is_whitespace());
                    if !project.is_empty() {
                        members.push(project.trim_start_matches(':').to_string());
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
            language: JAVA,
            build_system: GRADLE,
            runtime: JVM,
            package: project_name.map(|name| Package {
                name,
                version: None,
                is_application: false, // settings.gradle doesn't indicate runnability
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

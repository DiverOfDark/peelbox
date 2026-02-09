use crate::helpers::btree;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

pub struct BazelBuildParser;

impl ManifestParser for BazelBuildParser {
    fn filenames(&self) -> &[&str] {
        &["BUILD"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Look for cc_binary, java_binary, etc.
        if !content.contains("cc_binary")
            && !content.contains("java_binary")
            && !content.contains("py_binary")
            && !content.contains("go_binary")
        {
            return None;
        }

        // Extract binary name
        let name_re = regex::Regex::new(r#"name\s*=\s*"([^"]+)""#).ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        // Detect language from binary type
        let language = if content.contains("cc_binary") {
            LanguageId::Cpp
        } else if content.contains("java_binary") {
            LanguageId::Java
        } else if content.contains("py_binary") {
            LanguageId::Python
        } else {
            LanguageId::Go
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Bazel,
            runtime: RuntimeId::Native,
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
                    "bazel-7".into(),
                    "openjdk-21".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["bazel build //...".into()],
                member_transform: None,
                env: btree(&[
                    ("JAVA_HOME", "/usr/lib/jvm/java-21-openjdk"),
                    (
                        "PATH",
                        "/usr/lib/jvm/java-21-openjdk/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    ),
                ]),
                cache_dirs: vec![".cache/bazel".into()],
                artifacts: vec![(
                    format!("bazel-bin/{}", name),
                    format!("/app/{}", name),
                )],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(BazelBuildParser))
}

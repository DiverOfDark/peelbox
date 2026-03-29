use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const CPP: LanguageId = LanguageId::new("c++");
const JAVA: LanguageId = LanguageId::new("java");
const PYTHON: LanguageId = LanguageId::new("python");
const GO: LanguageId = LanguageId::new("go");
const BAZEL: BuildSystemId = BuildSystemId::new("bazel");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    BuildSystemMeta { slug: "bazel", display_name: "Bazel", aliases: &["bazel"] }
}

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
            CPP
        } else if content.contains("java_binary") {
            JAVA
        } else if content.contains("py_binary") {
            PYTHON
        } else {
            GO
        };

        // Language-specific build/runtime configuration
        // Note: Bazel creates `bazel-bin` as an absolute symlink to its cache directory
        // (e.g., /root/.cache/bazel/...); this symlink breaks during BuildKit's artifact
        // copy step because the target isn't under the /build mount. We resolve this by
        // adding a post-build `cp -rL` step that dereferences symlinks into `_output/`.
        let (build_commands, artifacts, runtime_packages, entrypoint, extra_build_packages) =
            if language == JAVA {
                // Java: build deploy JAR (self-contained uber JAR) for clean container packaging
                (
                    vec![format!(
                        "bazel build //:{n}_deploy.jar && mkdir -p _output && cp -rL bazel-bin/{n}_deploy.jar _output/",
                        n = name
                    )],
                    vec![(
                        format!("_output/{}_deploy.jar", name),
                        format!("/app/{}.jar", name),
                    )],
                    vec![
                        "openjdk-21-default-jvm".into(),
                        "openjdk-21-jre".into(),
                        "ca-certificates".into(),
                    ],
                    format!("java -jar /app/{}.jar", name),
                    vec![],
                )
            } else if language == PYTHON {
                // Python: copy binary + runfiles directory (Bazel py_binary needs both)
                (
                    vec![format!(
                        "bazel build //... && mkdir -p _output && cp -rL bazel-bin/{n} _output/ && cp -rL bazel-bin/{n}.runfiles _output/",
                        n = name
                    )],
                    vec![
                        (
                            format!("_output/{}", name),
                            format!("/app/{}", name),
                        ),
                        (
                            format!("_output/{}.runfiles/", name),
                            format!("/app/{}.runfiles/", name),
                        ),
                    ],
                    vec!["python".into(), "busybox".into(), "glibc".into(), "ca-certificates".into()],
                    format!("/app/{}", name),
                    vec!["python".into()],
                )
            } else {
                // C++ / Go: standalone binary
                (
                    vec![format!(
                        "bazel build //... && mkdir -p _output && cp -rL bazel-bin/{n} _output/",
                        n = name
                    )],
                    vec![(format!("_output/{}", name), format!("/app/{}", name))],
                    vec!["glibc".into(), "ca-certificates".into()],
                    format!("/app/{}", name),
                    vec![],
                )
            };

        let mut build_packages = vec![
            "build-base".into(),
            "bazel-7".into(),
            "openjdk-21".into(),
            "ca-certificates".into(),
        ];
        for pkg in extra_build_packages {
            if !build_packages.contains(&pkg) {
                build_packages.push(pkg);
            }
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BAZEL,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: build_packages,
                commands: build_commands,
                member_transform: None,
                env: btree(&[
                    ("JAVA_HOME", "/usr/lib/jvm/java-21-openjdk"),
                    (
                        "PATH",
                        "/usr/lib/jvm/java-21-openjdk/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    ),
                ]),
                cache_dirs: vec![".cache/bazel".into()],
                artifacts,
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: BTreeMap::new(),
                entrypoint: Some(entrypoint),
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

use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const ZIG: LanguageId = LanguageId::new("zig");
const ZIG_BS: BuildSystemId = BuildSystemId::new("zig");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "zig", display_name: "Zig", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "zig", display_name: "Zig Build", aliases: &["zig"] }
}

/// Parses build.zig.zon for package name and dependencies.
pub struct BuildZigZonParser;

impl ManifestParser for BuildZigZonParser {
    fn filenames(&self) -> &[&str] {
        &["build.zig.zon"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Extract .name from build.zig.zon (format: .name = .app, or .name = "app")
        let name = regex::Regex::new(r#"\.name\s*=\s*(?:\.(\w+)|"([^"]+)")"#)
            .ok()?
            .captures(content)
            .and_then(|c| {
                c.get(1)
                    .or_else(|| c.get(2))
                    .map(|m| m.as_str().to_string())
            })
            .unwrap_or_else(|| "app".to_string());

        // Parse dependencies from .dependencies section
        let mut deps = Vec::new();
        let dep_re = regex::Regex::new(r"\.(\w+)\s*=\s*\.\{").ok()?;
        let mut in_deps = false;
        let mut brace_depth = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains(".dependencies") && trimmed.contains("= .{") {
                in_deps = true;
                brace_depth = 1;
                continue;
            }
            if in_deps {
                // Check for dep names BEFORE counting braces on this line
                if brace_depth == 1 {
                    if let Some(cap) = dep_re.captures(trimmed) {
                        if let Some(dep_name) = cap.get(1) {
                            deps.push(Dependency {
                                name: dep_name.as_str().to_string(),
                                version: None,
                                scope: DepScope::Runtime,
                                is_internal: false,
                            });
                        }
                    }
                }
                for ch in trimmed.chars() {
                    if ch == '{' {
                        brace_depth += 1;
                    }
                    if ch == '}' {
                        brace_depth -= 1;
                    }
                }
                if brace_depth <= 0 {
                    in_deps = false;
                }
            }
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: ZIG,
            build_system: ZIG_BS,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec!["zig".into(), "build-base".into(), "ca-certificates".into()],
                commands: vec!["zig build -Doptimize=ReleaseSafe".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["zig-cache".into()],
                artifacts: vec![
                    (format!("zig-out/bin/{}", name), format!("/app/{}", name)),
                    ("zig-out/lib/".into(), "/app/lib/".into()),
                ],
                setup_commands: vec![],
                build_image: None,
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
    crate::registry::ManifestParserEntry(|| Box::new(BuildZigZonParser))
}

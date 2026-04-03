use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const GO: LanguageId = LanguageId::new("go");
const GO_SIMPLE: BuildSystemId = BuildSystemId::new("go-simple");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    BuildSystemMeta { slug: "go-simple", display_name: "Go (no module)", aliases: &["go-simple"] }
}

pub struct GoMainParser;

impl ManifestParser for GoMainParser {
    fn filenames(&self) -> &[&str] {
        &["main.go"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let parent = path.parent()?;
        // Check for go.mod in the same directory or any ancestor directory.
        // If a go.mod exists anywhere above, the GoModParser already handles
        // this project (including cmd/ subdirectories).
        if has_go_mod_in_ancestors(parent) {
            return None;
        }

        // Check content for `package main` and `func main()`
        let has_package_main = content.lines().any(|l| l.trim() == "package main");
        let has_func_main = content.lines().any(|l| {
            let trimmed = l.trim();
            trimmed == "func main() {" || trimmed.starts_with("func main()")
        });

        if !has_package_main || !has_func_main {
            return None;
        }

        let build_packages = vec!["go".into(), "git".into(), "ca-certificates".into()];

        let env_pairs: Vec<(&str, &str)> = vec![
            ("CGO_ENABLED", "0"),
            ("GOCACHE", "/app/.cache/go-build"),
            ("GOMODCACHE", "/app/.cache/go-mod"),
            ("GOSUMDB", "off"),
        ];

        Some(Manifest {
            path: path.to_path_buf(),
            language: GO,
            build_system: GO_SIMPLE,
            runtime: NATIVE,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: build_packages,
                commands: vec![
                    "go mod init app".into(),
                    "mkdir -p bin".into(),
                    "go build -o bin/app .".into(),
                ],
                member_transform: None,
                env: btree(&env_pairs),
                cache_dirs: vec![".cache/go-build".into(), ".cache/go-mod".into()],
                artifacts: vec![("bin/app".into(), "/app/app".into())],
                setup_commands: vec![],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some("/app/app".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Check if go.mod exists in the given directory or any ancestor.
fn has_go_mod_in_ancestors(dir: &Path) -> bool {
    let mut current = Some(dir);
    while let Some(d) = current {
        if d.join("go.mod").exists() {
            return true;
        }
        current = d.parent();
    }
    false
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(GoMainParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_go_main_parser_basic() {
        let parser = GoMainParser;
        let content = r#"package main

import "fmt"

func main() {
	fmt.Println("Hello from Go!")
}
"#;
        // Use a temp dir without go.mod
        let tmp = tempfile::tempdir().unwrap();
        let main_go = tmp.path().join("main.go");
        std::fs::write(&main_go, content).unwrap();

        let manifest = parser.parse(&main_go, content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("go"));
        assert_eq!(manifest.build_system, BuildSystemId::new("go-simple"));
        assert_eq!(manifest.runtime, RuntimeId::new("native"));

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(pkg.is_application);

        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("go build")));
        assert_eq!(manifest.runtime_config.entrypoint, Some("/app/app".into()));
    }

    #[test]
    fn test_go_main_parser_skips_with_go_mod() {
        let parser = GoMainParser;
        let content = r#"package main

import "fmt"

func main() {
	fmt.Println("Hello from Go!")
}
"#;
        // Create a temp dir WITH go.mod
        let tmp = tempfile::tempdir().unwrap();
        let main_go = tmp.path().join("main.go");
        std::fs::write(&main_go, content).unwrap();
        std::fs::write(
            tmp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.21\n",
        )
        .unwrap();

        let result = parser.parse(&main_go, content);
        assert!(result.is_none());
    }

    #[test]
    fn test_go_main_parser_not_main_package() {
        let parser = GoMainParser;
        let content = r#"package utils

func Helper() string {
	return "hello"
}
"#;
        let tmp = tempfile::tempdir().unwrap();
        let main_go = tmp.path().join("main.go");
        std::fs::write(&main_go, content).unwrap();

        let result = parser.parse(&main_go, content);
        assert!(result.is_none());
    }

    #[test]
    fn test_go_main_parser_no_func_main() {
        let parser = GoMainParser;
        let content = r#"package main

import "fmt"

func helper() {
	fmt.Println("Not main")
}
"#;
        let tmp = tempfile::tempdir().unwrap();
        let main_go = tmp.path().join("main.go");
        std::fs::write(&main_go, content).unwrap();

        let result = parser.parse(&main_go, content);
        assert!(result.is_none());
    }
}

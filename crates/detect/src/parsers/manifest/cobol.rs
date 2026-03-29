use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const COBOL: LanguageId = LanguageId::new("cobol");
const GNUCOBOL: BuildSystemId = BuildSystemId::new("gnucobol");
const NATIVE_RT: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "cobol", display_name: "COBOL", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "gnucobol", display_name: "GnuCOBOL", aliases: &["gnucobol"] }
}
inventory::submit! {
    RuntimeMeta { slug: "native", display_name: "Native", aliases: &[] }
}

pub struct CobolParser;

impl ManifestParser for CobolParser {
    fn filenames(&self) -> &[&str] {
        // Matched by extension in pipeline.rs classify_file (like .csproj)
        &[]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Verify this looks like COBOL source
        if !content.contains("IDENTIFICATION DIVISION")
            && !content.contains("PROGRAM-ID")
            && !content.contains("PROCEDURE DIVISION")
        {
            return None;
        }

        let parent = path.parent()?;

        // Find the main .cbl file: prefer index.cbl, then use this file
        let main_file = find_main_cbl(parent, path);

        let build_command = format!("cobc -x -o app {}", main_file);

        Some(Manifest {
            path: path.to_path_buf(),
            language: COBOL,
            build_system: GNUCOBOL,
            runtime: NATIVE_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["gnucobol".into(), "ca-certificates".into()],
                commands: vec![build_command],
                member_transform: None,
                env: btree(&[]),
                cache_dirs: vec![],
                artifacts: vec![(".".into(), "/app/".into())],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "libgcc".into(), "ca-certificates".into()],
                env: btree(&[]),
                entrypoint: Some("./app".into()),
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

/// Find the main .cbl file to compile.
/// Preference: index.cbl in same dir > current file.
/// Returns the path relative to the project directory for the build command.
fn find_main_cbl(parent: &Path, current_file: &Path) -> String {
    // Check for index.cbl in the same directory
    let index_path = parent.join("index.cbl");
    if index_path.exists() {
        return current_file
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                if n == "index.cbl" {
                    "index.cbl".to_string()
                } else {
                    "index.cbl".to_string()
                }
            })
            .unwrap_or_else(|| "index.cbl".to_string());
    }

    // Use the current file
    current_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("index.cbl")
        .to_string()
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(CobolParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_cobol_parser() {
        let parser = CobolParser;
        let content = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO-WORLD.

       PROCEDURE DIVISION.
       000-MAINLINE SECTION.
       000-START.
           DISPLAY "Hello from cobol!".
       000-EXIT.
       EXIT-PROGRAM."#;

        let manifest = parser
            .parse(Path::new("/tmp/test-cobol/index.cbl"), content)
            .unwrap();
        assert_eq!(manifest.language, COBOL);
        assert_eq!(manifest.build_system, GNUCOBOL);
        assert_eq!(manifest.runtime, NATIVE_RT);
        assert!(manifest.build.commands[0].contains("cobc -x -o app"));
        assert_eq!(manifest.runtime_config.entrypoint, Some("./app".into()));
    }

    #[test]
    fn test_cobol_rejects_non_cobol() {
        let parser = CobolParser;
        let result = parser.parse(Path::new("/tmp/test.cbl"), "Just some text");
        assert!(result.is_none());
    }

    #[test]
    fn test_cobol_free_format() {
        let parser = CobolParser;
        let content = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-WORLD.

PROCEDURE DIVISION.
000-MAINLINE SECTION.
000-START.
      DISPLAY "Hello from cobol! cobol-free".
000-EXIT.
EXIT-PROGRAM."#;

        let manifest = parser
            .parse(Path::new("/tmp/test-cobol/cobol-free.cbl"), content)
            .unwrap();
        assert_eq!(manifest.language, COBOL);
    }
}

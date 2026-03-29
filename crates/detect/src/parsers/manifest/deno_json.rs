use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const DENO: LanguageId = LanguageId::new("deno");
const DENO_BS: BuildSystemId = BuildSystemId::new("deno");
const DENO_RT: RuntimeId = RuntimeId::new("deno");

inventory::submit! {
    LanguageMeta { slug: "deno", display_name: "Deno", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "deno", display_name: "Deno", aliases: &["deno"] }
}
inventory::submit! {
    RuntimeMeta { slug: "deno", display_name: "Deno", aliases: &["deno"] }
}

pub struct DenoJsonParser;

impl ManifestParser for DenoJsonParser {
    fn filenames(&self) -> &[&str] {
        &["deno.json", "deno.jsonc"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Strip JSONC comments for parsing (handles //, /* */, and respects strings)
        let clean = strip_jsonc_comments(content);

        let json: serde_json::Value = serde_json::from_str(&clean).ok()?;

        let has_build = json.get("tasks").and_then(|t| t.get("build")).is_some();

        let mut build_commands = vec!["deno install".into()];
        if has_build {
            build_commands.push("deno task build".into());
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: DENO,
            build_system: DENO_BS,
            runtime: DENO_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["deno".into(), "ca-certificates".into()],
                commands: build_commands,
                member_transform: None,
                env: btree(&[("DENO_DIR", "/deno-dir")]),
                cache_dirs: vec!["/deno-dir".into()],
                artifacts: vec![
                    (".".into(), "/app".into()),
                    ("/deno-dir".into(), "/deno-dir".into()),
                ],
                setup_commands: vec![],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["deno".into(), "ca-certificates".into()],
                env: btree(&[("DENO_DIR", "/deno-dir")]),
                entrypoint: Some("deno run --allow-net --allow-read --allow-env main.ts".into()),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

/// Strip JSONC comments from content, handling:
/// - Single-line comments: `// ...`
/// - Multi-line comments: `/* ... */`
/// - Preserves `//` and `/*` inside JSON string literals
fn strip_jsonc_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        if in_string {
            // Inside a JSON string — pass through everything
            if chars[i] == '\\' && i + 1 < len {
                // Escaped character — emit both and skip
                result.push(chars[i]);
                result.push(chars[i + 1]);
                i += 2;
            } else if chars[i] == '"' {
                result.push(chars[i]);
                in_string = false;
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        } else if chars[i] == '"' {
            result.push(chars[i]);
            in_string = true;
            i += 1;
        } else if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            // Single-line comment — skip until end of line
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            // Keep the newline if present (helps preserve structure)
        } else if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            // Multi-line comment — skip until */
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                // Preserve newlines within multi-line comments to keep line structure
                if chars[i] == '\n' {
                    result.push('\n');
                }
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(DenoJsonParser))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_jsonc_single_line_comments() {
        let input = r#"{
  // this is a comment
  "key": "value"
}"#;
        let clean = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_strip_jsonc_multiline_comments() {
        let input = r#"{
  /* multiline
     comment */
  "key": "value"
}"#;
        let clean = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_strip_jsonc_inline_multiline() {
        let input = r#"{
  /* :) */ "key": "value"
}"#;
        let clean = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_strip_jsonc_preserves_strings() {
        let input = r#"{
  "url": "https://example.com/path"
}"#;
        let clean = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(parsed["url"], "https://example.com/path");
    }

    #[test]
    fn test_strip_jsonc_full_deno_jsonc() {
        let input = r#"// THIS FILE HAS ALL KINDS OF COMMENTS POSSIBLE TO TEST JSONC
{
  //some random comment
  "tasks": {
    // random afterline comment
    /* :) */ "dev": "deno run --watch main.ts",
    "start": "deno start main.ts"
  } /*
    This
    is some multiline comment
    */,
  "imports": {
    "@std/assert": "jsr:@std/assert@1"
  }
}"#;
        let clean = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(parsed["tasks"]["dev"], "deno run --watch main.ts");
        assert_eq!(parsed["tasks"]["start"], "deno start main.ts");
        assert_eq!(parsed["imports"]["@std/assert"], "jsr:@std/assert@1");
    }

    #[test]
    fn test_deno_jsonc_parser() {
        let input = r#"// THIS FILE HAS ALL KINDS OF COMMENTS POSSIBLE TO TEST JSONC
{
  //some random comment
  "tasks": {
    // random afterline comment
    /* :) */ "dev": "deno run --watch main.ts",
    "start": "deno start main.ts"
  } /*
    This
    is some multiline comment
    */,
  "imports": {
    "@std/assert": "jsr:@std/assert@1"
  }
}"#;
        let parser = DenoJsonParser;
        let manifest = parser.parse(Path::new("deno.jsonc"), input).unwrap();
        assert_eq!(manifest.language, crate::ids::LanguageId::new("deno"));
        assert_eq!(
            manifest.build_system,
            crate::ids::BuildSystemId::new("deno")
        );
        assert!(manifest.package.unwrap().is_application);
    }
}

use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const SCHEME: LanguageId = LanguageId::new("scheme");
const HAUNT: BuildSystemId = BuildSystemId::new("haunt");
const GUILE: RuntimeId = RuntimeId::new("guile");

inventory::submit! {
    LanguageMeta { slug: "scheme", display_name: "Scheme", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "haunt", display_name: "Haunt", aliases: &["haunt"] }
}
inventory::submit! {
    RuntimeMeta { slug: "guile", display_name: "Guile", aliases: &["guile"] }
}

pub struct SchemeHauntParser;

impl ManifestParser for SchemeHauntParser {
    fn filenames(&self) -> &[&str] {
        &["haunt.scm"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Verify it's a Haunt config
        if !content.contains("(use-modules (haunt") {
            return None;
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: SCHEME,
            build_system: HAUNT,
            runtime: GUILE,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "guile".into(),
                    "curl".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "guile -c '(use-modules (guix packages))' 2>/dev/null || (curl -fsSL https://git.savannah.gnu.org/cgit/haunt.git/snapshot/haunt-0.3.0.tar.gz | tar -xz && cd haunt-0.3.0 && ./configure && make install && cd .. && rm -rf haunt-0.3.0)".into(),
                    "haunt build".into(),
                ],
                member_transform: None,
                env: btree(&[]),
                cache_dirs: vec![],
                artifacts: vec![("site".into(), "/app/site".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "busybox".into(),
                    "ca-certificates".into(),
                ],
                env: btree(&[]),
                entrypoint: Some("busybox httpd -f -p 8080 -h /app/site".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(SchemeHauntParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_haunt_parser() {
        let parser = SchemeHauntParser;
        let content = r#"(use-modules (haunt asset)
             (haunt builder blog)
             (haunt builder assets)
             (haunt reader commonmark)
             (haunt site))

(site #:title "Built with Guile"
      #:domain "example.com"
      #:default-metadata
      '((author . "John Doe"))
      #:readers (list)
      #:builders (list)
)"#;

        let manifest = parser
            .parse(Path::new("haunt.scm"), content)
            .unwrap();
        assert_eq!(manifest.language, SCHEME);
        assert_eq!(manifest.build_system, HAUNT);
        assert_eq!(manifest.runtime, GUILE);
        assert!(manifest.build.commands.iter().any(|c| c.contains("haunt build")));
        assert!(manifest.runtime_config.entrypoint.as_ref().unwrap().contains("httpd"));
        assert_eq!(manifest.runtime_config.ports, vec![8080]);
    }

    #[test]
    fn test_haunt_rejects_non_haunt() {
        let parser = SchemeHauntParser;
        let content = "(display \"Hello from Scheme!\")";
        let result = parser.parse(Path::new("haunt.scm"), content);
        assert!(result.is_none());
    }
}

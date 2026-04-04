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

        // Haunt is a pure Guile Scheme static site generator.
        // We install Guile via Wolfi packages, then extract Haunt and
        // guile-commonmark tarballs, create the haunt script by filling in template
        // placeholders (bypassing ./configure which also uses Guile), and set
        // GUILE_LOAD_PATH so all modules are found.
        let install_haunt = concat!(
            "mkdir -p /usr/local/bin && ",
            "curl -fsSL https://files.dthompson.us/haunt/haunt-0.3.0.tar.gz | tar -xz",
            " && curl -fsSL https://github.com/OrangeShark/guile-commonmark/archive/refs/tags/v0.1.2.tar.gz | tar -xz",
            " && sed 's|@GUILE@|/usr/bin/guile|g; ",
            "s|@guilemoduledir@|/app/haunt-0.3.0|g; ",
            "s|@guileobjectdir@|/app/haunt-0.3.0|g' ",
            "haunt-0.3.0/scripts/haunt.in > /usr/local/bin/haunt && chmod +x /usr/local/bin/haunt",
            " && printf '(define-module (haunt config)\\n",
            "  #:export (%%haunt-version %%rsync %%hut %%tar))\\n",
            "(define %%haunt-version \"0.3.0\")\\n",
            "(define %%rsync \"rsync\")\\n",
            "(define %%hut \"hut\")\\n",
            "(define %%tar \"tar\")\\n' > haunt-0.3.0/haunt/config.scm",
        );

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
                packages: vec!["guile".into(), "curl".into(), "ca-certificates".into()],
                commands: vec![install_haunt.into(), "haunt build && mkdir -p site".into()],
                member_transform: None,
                env: btree(&[
                    ("GUILE_AUTO_COMPILE", "0"),
                    (
                        "GUILE_LOAD_PATH",
                        "/app/haunt-0.3.0:/app/guile-commonmark-0.1.2",
                    ),
                ]),
                cache_dirs: vec![],
                artifacts: vec![("site".into(), "/app/site".into())],
                build_image: None,
                asset_build: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["busybox".into(), "ca-certificates".into()],
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

        let manifest = parser.parse(Path::new("haunt.scm"), content).unwrap();
        assert_eq!(manifest.language, SCHEME);
        assert_eq!(manifest.build_system, HAUNT);
        assert_eq!(manifest.runtime, GUILE);
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("haunt build")));
        assert!(manifest
            .runtime_config
            .entrypoint
            .as_ref()
            .unwrap()
            .contains("httpd"));
        assert_eq!(manifest.runtime_config.ports, vec![8080]);
        assert!(manifest.build.packages.contains(&"guile".to_string()));
    }

    #[test]
    fn test_haunt_rejects_non_haunt() {
        let parser = SchemeHauntParser;
        let content = "(display \"Hello from Scheme!\")";
        let result = parser.parse(Path::new("haunt.scm"), content);
        assert!(result.is_none());
    }
}

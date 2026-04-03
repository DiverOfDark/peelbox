use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const STATIC: LanguageId = LanguageId::new("static");
const STATIC_BS: BuildSystemId = BuildSystemId::new("static");
const STATIC_RT: RuntimeId = RuntimeId::new("static");

inventory::submit! {
    LanguageMeta { slug: "static", display_name: "Static", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "static", display_name: "Static", aliases: &[] }
}
inventory::submit! {
    RuntimeMeta { slug: "static", display_name: "Static", aliases: &[] }
}

pub struct StaticfileParser;

impl ManifestParser for StaticfileParser {
    fn filenames(&self) -> &[&str] {
        &["Staticfile", "index.html"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let filename = path.file_name()?.to_str()?;

        if filename == "Staticfile" {
            return parse_staticfile(path, content);
        }

        if filename == "index.html" {
            return parse_index_html(path, content);
        }

        None
    }
}

fn parse_staticfile(path: &Path, content: &str) -> Option<Manifest> {
    // Parse root directive from Staticfile
    let root = content
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("root:") {
                Some(trimmed.strip_prefix("root:")?.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let entrypoint = if root.is_empty() {
        "busybox httpd -f -p 8080 -h /app".to_string()
    } else {
        format!("busybox httpd -f -p 8080 -h /app/{}", root)
    };

    Some(Manifest {
        path: path.to_path_buf(),
        language: STATIC,
        build_system: STATIC_BS,
        runtime: STATIC_RT,
        package: Some(Package {
            name: "app".to_string(),
            version: None,
            is_application: true,
        }),
        workspace: None,
        dependencies: Vec::new(),
        build: BuildSpec {
            packages: vec!["ca-certificates".into()],
            commands: vec!["echo 'No build step required'".into()],
            member_transform: None,
            env: btree(&[]),
            cache_dirs: vec![],
            artifacts: vec![(".".into(), "/app/".into())],
            build_image: None,
        },
        runtime_config: RuntimeSpec {
            packages: vec!["busybox".into(), "ca-certificates".into()],
            env: btree(&[]),
            entrypoint: Some(entrypoint),
            workdir: Some("/app".into()),
            ports: vec![8080],
            health_endpoint: None,
        },
    })
}

fn parse_index_html(path: &Path, content: &str) -> Option<Manifest> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("<!DOCTYPE")
        && !trimmed.starts_with("<!doctype")
        && !trimmed.starts_with("<html")
        && !trimmed.starts_with("<HTML")
    {
        return None;
    }

    // Don't match index.html if it's in a subdirectory of something already detected
    // (e.g., site/index.html in a staticfile project)
    let parent = path.parent()?;

    // Check that this directory and parent directories don't have other manifest files
    // that would indicate a more specific project type
    let known_manifests = [
        "package.json",
        "Cargo.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "Staticfile",
    ];
    let dirs_to_check: Vec<&Path> = {
        let mut dirs = vec![parent];
        if let Some(grandparent) = parent.parent() {
            dirs.push(grandparent);
        }
        dirs
    };
    for dir in &dirs_to_check {
        for manifest_name in &known_manifests {
            if dir.join(manifest_name).exists() {
                return None;
            }
        }
    }

    Some(Manifest {
        path: path.to_path_buf(),
        language: STATIC,
        build_system: STATIC_BS,
        runtime: STATIC_RT,
        package: Some(Package {
            name: "app".to_string(),
            version: None,
            is_application: true,
        }),
        workspace: None,
        dependencies: Vec::new(),
        build: BuildSpec {
            packages: vec!["ca-certificates".into()],
            commands: vec!["echo 'No build step required'".into()],
            member_transform: None,
            env: btree(&[]),
            cache_dirs: vec![],
            artifacts: vec![(".".into(), "/app/".into())],
            build_image: None,
        },
        runtime_config: RuntimeSpec {
            packages: vec!["busybox".into(), "ca-certificates".into()],
            env: btree(&[]),
            entrypoint: Some("busybox httpd -f -p 8080 -h /app".into()),
            workdir: Some("/app".into()),
            ports: vec![8080],
            health_endpoint: None,
        },
    })
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(StaticfileParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_staticfile_with_root() {
        let parser = StaticfileParser;
        let content = "root: site\n";
        let manifest = parser.parse(Path::new("/tmp/Staticfile"), content).unwrap();
        assert_eq!(manifest.language, STATIC);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("busybox httpd -f -p 8080 -h /app/site".into())
        );
    }

    #[test]
    fn test_staticfile_no_root() {
        let parser = StaticfileParser;
        let content = "";
        let manifest = parser.parse(Path::new("/tmp/Staticfile"), content).unwrap();
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("busybox httpd -f -p 8080 -h /app".into())
        );
    }

    #[test]
    fn test_index_html() {
        let parser = StaticfileParser;
        let content = "<!DOCTYPE html>\n<html><body>Hello</body></html>";
        let manifest = parser
            .parse(Path::new("/tmp/test-static/index.html"), content)
            .unwrap();
        assert_eq!(manifest.language, STATIC);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("busybox httpd -f -p 8080 -h /app".into())
        );
    }

    #[test]
    fn test_index_html_not_html() {
        let parser = StaticfileParser;
        let content = "This is not HTML content";
        let result = parser.parse(Path::new("/tmp/test/index.html"), content);
        assert!(result.is_none());
    }
}

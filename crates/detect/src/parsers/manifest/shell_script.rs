use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const SHELL: LanguageId = LanguageId::new("shell");
const SHELL_BS: BuildSystemId = BuildSystemId::new("shell");
const SHELL_RT: RuntimeId = RuntimeId::new("shell");

inventory::submit! {
    LanguageMeta { slug: "shell", display_name: "Shell", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "shell", display_name: "Shell", aliases: &[] }
}
inventory::submit! {
    RuntimeMeta { slug: "shell", display_name: "Shell", aliases: &[] }
}

pub struct ShellScriptParser;

impl ManifestParser for ShellScriptParser {
    fn filenames(&self) -> &[&str] {
        &["start.sh"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Check content looks like a shell script
        let trimmed = content.trim();
        let is_shell = trimmed.starts_with("#!")
            || trimmed.contains("echo ")
            || trimmed.contains("export ")
            || trimmed.contains("if [")
            || trimmed.contains("for ")
            || trimmed.contains("while ")
            || trimmed.contains("exec ")
            || trimmed.contains(" | ")
            || trimmed.contains("$(")
            || trimmed.contains("&&")
            || trimmed.contains("find ")
            || trimmed.contains("cd ")
            || trimmed.contains("uname")
            || trimmed.contains("$@")
            || trimmed.contains("${");

        if !is_shell {
            return None;
        }

        // Detect bash vs sh from shebang
        let needs_bash = content.starts_with("#!/bin/bash")
            || content.starts_with("#!/usr/bin/env bash")
            || content.contains("[[")
            || content.contains("${!");

        let mut runtime_packages = vec!["busybox".into()];
        if needs_bash {
            runtime_packages.push("bash".into());
        }
        runtime_packages.push("ca-certificates".into());

        let filename = path.file_name()?.to_str()?;

        Some(Manifest {
            path: path.to_path_buf(),
            language: SHELL,
            build_system: SHELL_BS,
            runtime: SHELL_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["ca-certificates".into()],
                commands: vec![format!("chmod +x {}", filename)],
                member_transform: None,
                env: btree(&[]),
                cache_dirs: vec![],
                artifacts: vec![(".".into(), "/app/".into())],
                            build_image: None,
},
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: btree(&[]),
                entrypoint: Some(format!("./{}", filename)),
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ShellScriptParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_shell_script_basic() {
        let parser = ShellScriptParser;
        let content = "#!/bin/sh\necho \"Hello, World!\"";
        let manifest = parser
            .parse(Path::new("/tmp/start.sh"), content)
            .unwrap();
        assert_eq!(manifest.language, SHELL);
        assert_eq!(manifest.build_system, SHELL_BS);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("./start.sh".into())
        );
        // sh script should not include bash
        assert!(!manifest.runtime_config.packages.contains(&"bash".into()));
    }

    #[test]
    fn test_shell_script_bash() {
        let parser = ShellScriptParser;
        let content = "#!/bin/bash\necho \"Hello from bash\"";
        let manifest = parser
            .parse(Path::new("/tmp/start.sh"), content)
            .unwrap();
        assert!(manifest.runtime_config.packages.contains(&"bash".into()));
    }

    #[test]
    fn test_shell_script_not_shell() {
        let parser = ShellScriptParser;
        let content = "This is just a text file with no shell commands.";
        let result = parser.parse(Path::new("/tmp/start.sh"), content);
        assert!(result.is_none());
    }
}

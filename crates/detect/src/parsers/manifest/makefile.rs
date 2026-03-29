use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const C: LanguageId = LanguageId::new("c");
const CPP: LanguageId = LanguageId::new("c++");
const MAKE: BuildSystemId = BuildSystemId::new("make");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    BuildSystemMeta { slug: "make", display_name: "Make", aliases: &["make"] }
}
inventory::submit! {
    LanguageMeta { slug: "c", display_name: "C", aliases: &[] }
}

pub struct MakefileParser;

impl ManifestParser for MakefileParser {
    fn filenames(&self) -> &[&str] {
        &["Makefile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains(':') {
            return None;
        }

        // Detect language: look for C++ indicators, otherwise default to C
        let is_cpp = content.contains("CXX")
            || content.contains("g++")
            || content.contains("c++")
            || content.contains("clang++")
            || content.contains(".cpp")
            || content.contains(".cxx")
            || content.contains(".cc");

        let language = if is_cpp { CPP } else { C };

        // Extract target name from common Makefile patterns:
        // TARGET = name, TARGET := name, BINARY = name, APP = name, PROGRAM = name
        let target_re =
            regex::Regex::new(r"(?m)^(?:TARGET|BINARY|APP|PROGRAM|BIN)\s*[:?]?=\s*(\S+)").ok()?;
        let name = target_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| {
                let raw = m.as_str();
                // Strip $(...) variable references like $(NAME) to get plain names
                if raw.starts_with("$(") || raw.starts_with("${") {
                    "app".to_string()
                } else {
                    raw.to_string()
                }
            })
            .unwrap_or_else(|| "app".to_string());

        // Detect if there's an install target
        let has_install = regex::Regex::new(r"(?m)^install\s*:")
            .ok()
            .map(|r| r.is_match(content))
            .unwrap_or(false);

        let build_commands = if has_install {
            vec!["make".into(), "make install DESTDIR=/out".into()]
        } else {
            vec!["make".into()]
        };

        // Build packages: gcc provides both C and C++ compilers in Wolfi
        let build_packages = vec![
            "make".into(),
            "build-base".into(),
            "gcc".into(),
            "ca-certificates".into(),
        ];

        // Runtime packages for native binaries
        let mut runtime_packages = vec!["glibc".into(), "ca-certificates".into()];
        if is_cpp {
            runtime_packages.push("libstdc++".into());
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: MAKE,
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
                env: BTreeMap::new(),
                cache_dirs: Vec::new(),
                artifacts: vec![(name.clone(), format!("/app/{}", name))],
                setup_commands: vec![],
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
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
    crate::registry::ManifestParserEntry(|| Box::new(MakefileParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;
    use std::path::Path;

    #[test]
    fn test_makefile_parser_cpp_project() {
        let parser = MakefileParser;
        let content = r#"CXX = g++
CXXFLAGS = -std=c++17 -O2 -Wall
TARGET = app

all: $(TARGET)

$(TARGET): main.cpp
	$(CXX) $(CXXFLAGS) -o $(TARGET) main.cpp -lpthread

clean:
	rm -f $(TARGET)
"#;
        let manifest = parser.parse(Path::new("Makefile"), content).unwrap();
        assert_eq!(manifest.language, CPP);
        assert_eq!(manifest.build_system, MAKE);
        assert_eq!(manifest.runtime, NATIVE);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(pkg.is_application);
        assert_eq!(manifest.build.commands, vec!["make"]);
        assert_eq!(
            manifest.build.artifacts,
            vec![("app".into(), "/app/app".into())]
        );
        assert_eq!(manifest.runtime_config.entrypoint, Some("/app/app".into()));
        assert!(manifest
            .runtime_config
            .packages
            .contains(&"libstdc++".into()));
    }

    #[test]
    fn test_makefile_parser_c_project() {
        let parser = MakefileParser;
        let content = r#"CC = gcc
CFLAGS = -O2 -Wall
TARGET = myserver

all: $(TARGET)

$(TARGET): src/main.c
	$(CC) $(CFLAGS) -o $(TARGET) src/main.c

install:
	install -D $(TARGET) $(DESTDIR)/usr/local/bin/$(TARGET)

clean:
	rm -f $(TARGET)
"#;
        let manifest = parser.parse(Path::new("Makefile"), content).unwrap();
        assert_eq!(manifest.language, C);
        assert_eq!(manifest.build_system, MAKE);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "myserver");
        // With install target, should have install command
        assert_eq!(
            manifest.build.commands,
            vec!["make", "make install DESTDIR=/out"]
        );
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("/app/myserver".into())
        );
        // Pure C should not have libstdc++
        assert!(!manifest
            .runtime_config
            .packages
            .contains(&"libstdc++".into()));
    }

    #[test]
    fn test_makefile_parser_no_target_variable() {
        let parser = MakefileParser;
        let content = r#"all: main
	gcc -o main main.c

clean:
	rm -f main
"#;
        let manifest = parser.parse(Path::new("Makefile"), content).unwrap();
        assert_eq!(manifest.language, C);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app"); // Defaults to "app" when no TARGET variable
    }

    #[test]
    fn test_makefile_parser_rejects_empty() {
        let parser = MakefileParser;
        assert!(parser.parse(Path::new("Makefile"), "").is_none());
    }

    #[test]
    fn test_makefile_parser_variable_reference_target() {
        let parser = MakefileParser;
        let content = r#"TARGET = $(PROJECT_NAME)

all: $(TARGET)
	gcc -o $(TARGET) main.c
"#;
        let manifest = parser.parse(Path::new("Makefile"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app"); // Variable references fall back to "app"
    }
}

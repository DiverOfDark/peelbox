//! Manifest file parsers.
//!
//! Each parser reads a specific manifest format and normalizes it into a generic `Manifest`.

mod bazel_build;
mod build_gradle;
mod build_zig_zon;
mod cargo_toml;
mod cmake_lists;
mod composer_json;
mod csproj;
mod deno_json;
mod gemfile;
mod go_mod;
mod go_work;
mod index_php;
mod makefile;
mod meson_build;
mod mix_exs;
mod package_json;
mod pipfile;
mod pnpm_lock;
mod pom_xml;
mod project_clj;
mod pyproject_toml;
mod requirements_txt;
mod settings_gradle;
mod setup_py;
mod uv_lock;
mod yarn_lock;
mod zig_build;

pub use bazel_build::BazelBuildParser;
pub use build_gradle::BuildGradleParser;
pub use build_zig_zon::BuildZigZonParser;
pub use cargo_toml::CargoTomlParser;
pub use cmake_lists::CMakeListsParser;
pub use composer_json::ComposerJsonParser;
pub use csproj::CsprojParser;
pub use deno_json::DenoJsonParser;
pub use gemfile::GemfileParser;
pub use go_mod::GoModParser;
pub use go_work::GoWorkParser;
pub use index_php::IndexPhpParser;
pub use makefile::MakefileParser;
pub use meson_build::MesonBuildParser;
pub use mix_exs::MixExsParser;
pub use package_json::PackageJsonParser;
pub use pipfile::PipfileParser;
pub use pnpm_lock::PnpmLockParser;
pub use pom_xml::PomXmlParser;
pub use project_clj::ProjectCljParser;
pub use pyproject_toml::PyProjectTomlParser;
pub use requirements_txt::RequirementsTxtParser;
pub use settings_gradle::SettingsGradleParser;
pub use setup_py::SetupPyParser;
pub use uv_lock::UvLockParser;
pub use yarn_lock::YarnLockParser;
pub use zig_build::ZigBuildParser;

use crate::types::*;

/// Parse npm-style dependencies from a JSON value (shared by PackageJson, YarnLock, PnpmLock).
pub(crate) fn parse_npm_deps(json: &serde_json::Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for (section, scope) in &[
        ("dependencies", DepScope::Runtime),
        ("devDependencies", DepScope::Dev),
        ("peerDependencies", DepScope::Peer),
    ] {
        if let Some(obj) = json.get(section).and_then(|v| v.as_object()) {
            for (name, ver) in obj {
                deps.push(Dependency {
                    name: name.clone(),
                    version: ver.as_str().map(String::from),
                    scope: scope.clone(),
                    is_internal: false,
                });
            }
        }
    }
    deps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_cargo_toml_parser_simple() {
        let parser = CargoTomlParser;
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("Cargo.toml"), content)
                .unwrap();
        assert_eq!(manifest.language, crate::ids::LanguageId::new("rust"));
        assert_eq!(
            manifest.build_system,
            crate::ids::BuildSystemId::new("cargo")
        );
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert_eq!(manifest.dependencies.len(), 2);
    }

    #[test]
    fn test_cargo_toml_parser_workspace() {
        let parser = CargoTomlParser;
        let content = r#"
[workspace]
members = ["crates/*", "apps/cli"]
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("Cargo.toml"), content)
                .unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["crates/*", "apps/cli"]);
        assert!(manifest.package.is_none());
    }

    #[test]
    fn test_package_json_parser() {
        let parser = PackageJsonParser;
        let content = r#"{
            "name": "my-app",
            "version": "1.0.0",
            "scripts": { "start": "node index.js", "build": "tsc" },
            "dependencies": { "express": "^4.0.0" },
            "devDependencies": { "typescript": "^5.0.0" }
        }"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("package.json"), content)
                .unwrap();
        assert_eq!(manifest.language, crate::ids::LanguageId::new("javascript"));
        assert_eq!(manifest.build_system, crate::ids::BuildSystemId::new("npm"));
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert!(pkg.is_application);
    }

    #[test]
    fn test_package_json_pnpm() {
        let parser = PackageJsonParser;
        let content = r#"{
            "name": "my-app",
            "packageManager": "pnpm@8.0.0",
            "dependencies": { "express": "^4.0.0" }
        }"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("package.json"), content)
                .unwrap();
        assert_eq!(
            manifest.build_system,
            crate::ids::BuildSystemId::new("pnpm")
        );
    }

    #[test]
    fn test_pom_xml_parser() {
        let parser = PomXmlParser;
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
    <properties>
        <java.version>21</java.version>
    </properties>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("pom.xml"), content).unwrap();
        assert_eq!(manifest.language, crate::ids::LanguageId::new("java"));
        assert_eq!(
            manifest.build_system,
            crate::ids::BuildSystemId::new("maven")
        );
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name.contains("spring-boot-starter-web")));
    }

    #[test]
    fn test_go_mod_parser() {
        let parser = GoModParser;
        let content = r#"module github.com/example/myserver

go 1.21

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/lib/pq v1.10.9
)"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("go.mod"), content).unwrap();
        assert_eq!(manifest.language, crate::ids::LanguageId::new("go"));
        assert_eq!(
            manifest.build_system,
            crate::ids::BuildSystemId::new("go-mod")
        );
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "myserver");
        assert_eq!(manifest.dependencies.len(), 2);
    }

    #[test]
    fn test_settings_gradle_parser() {
        let parser = SettingsGradleParser;
        let content = r#"
rootProject.name = 'my-project'
include('core', 'web', 'api-service')
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("settings.gradle"), content)
                .unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["core", "web", "api-service"]);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-project");
    }

    #[test]
    fn test_pyproject_toml_uv_workspace() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "my-workspace"
version = "0.1.0"
requires-python = ">=3.12"

[tool.uv.workspace]
members = ["packages/*"]
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("pyproject.toml"), content)
                .unwrap();
        assert_eq!(manifest.language, crate::ids::LanguageId::new("python"));
        assert_eq!(manifest.build_system, crate::ids::BuildSystemId::new("uv"));
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["packages/*"]);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-workspace");
        // Should have UV build commands
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("uv sync")));
        assert!(manifest.build.member_transform.is_some());
    }

    #[test]
    fn test_pyproject_toml_uv_without_workspace() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "my-app"
version = "1.0.0"
dependencies = ["flask>=3.0.0"]

[tool.uv]
dev-dependencies = ["pytest"]
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("pyproject.toml"), content)
                .unwrap();
        assert_eq!(manifest.build_system, crate::ids::BuildSystemId::new("uv"));
        assert!(manifest.workspace.is_none());
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("uv sync")));
        assert!(manifest.dependencies.iter().any(|d| d.name == "flask"));
    }

    #[test]
    fn test_pyproject_toml_plain_pip() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "my-app"
version = "1.0.0"
dependencies = ["requests>=2.0"]
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("pyproject.toml"), content)
                .unwrap();
        assert_eq!(manifest.build_system, crate::ids::BuildSystemId::new("pip"));
        assert!(manifest.workspace.is_none());
        // Should NOT have UV commands
        assert!(!manifest.build.commands.iter().any(|c| c.contains("uv")));
    }

    #[test]
    fn test_pyproject_toml_uv_workspace_multiple_members() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "monorepo"
version = "0.1.0"

[tool.uv.workspace]
members = ["packages/*", "apps/*", "libs/core"]
"#;
        let manifest =
            crate::traits::ManifestParser::parse(&parser, Path::new("pyproject.toml"), content)
                .unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["packages/*", "apps/*", "libs/core"]);
    }
}

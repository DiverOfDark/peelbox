use super::structure::Service;
use crate::pipeline::Confidence;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInfo {
    pub cache_dirs: Vec<PathBuf>,
    pub confidence: Confidence,
}

pub fn execute(service: &Service) -> CacheInfo {
    use crate::stack::BuildSystemId;
    let cache_dirs = match service.build_system {
        BuildSystemId::Npm | BuildSystemId::Yarn | BuildSystemId::Pnpm | BuildSystemId::Bun => {
            vec![
                PathBuf::from("node_modules"),
                PathBuf::from(".npm"),
                PathBuf::from(".pnpm-store"),
                PathBuf::from(".yarn/cache"),
            ]
        }
        BuildSystemId::Cargo => vec![PathBuf::from("target")],
        BuildSystemId::Maven => vec![PathBuf::from(".m2/repository"), PathBuf::from("target")],
        BuildSystemId::Gradle => vec![PathBuf::from(".gradle"), PathBuf::from("build")],
        BuildSystemId::GoMod => vec![PathBuf::from("go/pkg/mod")],
        BuildSystemId::Pip | BuildSystemId::Poetry => vec![
            PathBuf::from("__pycache__"),
            PathBuf::from(".venv"),
            PathBuf::from("venv"),
        ],
        BuildSystemId::Composer => vec![PathBuf::from("vendor")],
        BuildSystemId::Bundler => vec![PathBuf::from("vendor/bundle")],
        BuildSystemId::Mix => vec![PathBuf::from("_build"), PathBuf::from("deps")],
        BuildSystemId::DotNet => vec![PathBuf::from("obj"), PathBuf::from("bin")],
        BuildSystemId::CMake => vec![PathBuf::from("build")],
        BuildSystemId::Pipenv => vec![],
    };

    CacheInfo {
        cache_dirs,
        confidence: Confidence::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_npm() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let result = execute(&service);
        assert!(result.cache_dirs.contains(&PathBuf::from("node_modules")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_cache_cargo() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        let result = execute(&service);
        assert_eq!(result.cache_dirs, vec![PathBuf::from("target")]);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_cache_maven() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "pom.xml".to_string(),
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Maven,
        };

        let result = execute(&service);
        assert!(result.cache_dirs.contains(&PathBuf::from(".m2/repository")));
        assert!(result.cache_dirs.contains(&PathBuf::from("target")));
    }

    #[test]
    fn test_cache_gradle() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "build.gradle".to_string(),
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Gradle,
        };

        let result = execute(&service);
        assert!(result.cache_dirs.contains(&PathBuf::from(".gradle")));
        assert!(result.cache_dirs.contains(&PathBuf::from("build")));
    }

    #[test]
    fn test_cache_go() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "go.mod".to_string(),
            language: crate::stack::LanguageId::Go,
            build_system: crate::stack::BuildSystemId::GoMod,
        };

        let result = execute(&service);
        assert_eq!(result.cache_dirs, vec![PathBuf::from("go/pkg/mod")]);
    }

    #[test]
    fn test_cache_unknown() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "unknown.txt".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Pipenv,
        };

        let result = execute(&service);
        assert!(result.cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }
}

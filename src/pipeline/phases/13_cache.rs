use super::structure::Service;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInfo {
    pub cache_dirs: Vec<PathBuf>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

pub fn execute(service: &Service) -> CacheInfo {
    let cache_dirs = match service.build_system.as_str() {
        "npm" | "yarn" | "pnpm" | "bun" => vec![
            PathBuf::from("node_modules"),
            PathBuf::from(".npm"),
            PathBuf::from(".pnpm-store"),
            PathBuf::from(".yarn/cache"),
        ],
        "cargo" => vec![PathBuf::from("target")],
        "maven" => vec![PathBuf::from(".m2/repository"), PathBuf::from("target")],
        "gradle" => vec![PathBuf::from(".gradle"), PathBuf::from("build")],
        "go" => vec![PathBuf::from("go/pkg/mod")],
        "pip" | "poetry" => vec![
            PathBuf::from("__pycache__"),
            PathBuf::from(".venv"),
            PathBuf::from("venv"),
        ],
        "composer" => vec![PathBuf::from("vendor")],
        "bundler" => vec![PathBuf::from("vendor/bundle")],
        "mix" => vec![PathBuf::from("_build"), PathBuf::from("deps")],
        "dotnet" => vec![PathBuf::from("obj"), PathBuf::from("bin")],
        "cmake" | "make" => vec![PathBuf::from("build")],
        _ => vec![],
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
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
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
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
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
            language: "Java".to_string(),
            build_system: "maven".to_string(),
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
            language: "Java".to_string(),
            build_system: "gradle".to_string(),
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
            language: "Go".to_string(),
            build_system: "go".to_string(),
        };

        let result = execute(&service);
        assert_eq!(result.cache_dirs, vec![PathBuf::from("go/pkg/mod")]);
    }

    #[test]
    fn test_cache_unknown() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "unknown.txt".to_string(),
            language: "Unknown".to_string(),
            build_system: "unknown".to_string(),
        };

        let result = execute(&service);
        assert!(result.cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }
}

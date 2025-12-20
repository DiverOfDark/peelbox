//! Unified stack registry for languages, build systems, frameworks, and orchestrators.
//!
//! Type-safe registry system with strongly-typed identifiers (LanguageId, BuildSystemId,
//! FrameworkId, OrchestratorId) providing compile-time validation.
//!
//! # Example
//!
//! ```no_run
//! use aipack::stack::{StackRegistry, BuildSystemId, LanguageId};
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! let registry = StackRegistry::with_defaults();
//!
//! let manifest_path = Path::new("Cargo.toml");
//! let content = std::fs::read_to_string(manifest_path)?;
//! let stack = registry.detect_stack(manifest_path, &content).unwrap();
//!
//! let build_system = registry.get_build_system(BuildSystemId::Cargo).unwrap();
//! let language = registry.get_language(LanguageId::Rust).unwrap();
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

pub mod buildsystem;
pub mod detection;
pub mod framework;
pub mod language;
pub mod orchestrator;
pub mod registry;
pub mod runtime;

pub use buildsystem::{BuildSystem, BuildTemplate, ManifestPattern};
pub use detection::DetectionStack;
pub use framework::{DependencyPattern, DependencyPatternType, Framework};
pub use language::{
    Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition,
};
pub use orchestrator::{MonorepoOrchestrator, OrchestratorId};
pub use registry::StackRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageId {
    Rust,
    Java,
    Kotlin,
    JavaScript,
    TypeScript,
    Python,
    Go,
    #[serde(rename = "csharp")]
    CSharp,
    #[serde(rename = "fsharp")]
    FSharp,
    Ruby,
    #[serde(rename = "php")]
    PHP,
    #[serde(rename = "c++")]
    Cpp,
    Elixir,
}

impl LanguageId {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::CSharp => "C#",
            Self::FSharp => "F#",
            Self::Ruby => "Ruby",
            Self::PHP => "PHP",
            Self::Cpp => "C++",
            Self::Elixir => "Elixir",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Rust" => Some(Self::Rust),
            "Java" => Some(Self::Java),
            "Kotlin" => Some(Self::Kotlin),
            "JavaScript" => Some(Self::JavaScript),
            "TypeScript" => Some(Self::TypeScript),
            "Python" => Some(Self::Python),
            "Go" => Some(Self::Go),
            "C#" => Some(Self::CSharp),
            "F#" => Some(Self::FSharp),
            "Ruby" => Some(Self::Ruby),
            "PHP" => Some(Self::PHP),
            "C++" => Some(Self::Cpp),
            "Elixir" => Some(Self::Elixir),
            _ => None,
        }
    }

    pub fn all_variants() -> &'static [Self] {
        &[
            Self::Rust,
            Self::Java,
            Self::Kotlin,
            Self::JavaScript,
            Self::TypeScript,
            Self::Python,
            Self::Go,
            Self::CSharp,
            Self::FSharp,
            Self::Ruby,
            Self::PHP,
            Self::Cpp,
            Self::Elixir,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildSystemId {
    Cargo,
    Maven,
    Gradle,
    #[serde(rename = "npm")]
    Npm,
    Yarn,
    #[serde(rename = "pnpm")]
    Pnpm,
    Bun,
    Pip,
    Poetry,
    Pipenv,
    #[serde(rename = "go-mod")]
    GoMod,
    #[serde(rename = "dotnet")]
    DotNet,
    Composer,
    Bundler,
    #[serde(rename = "cmake")]
    CMake,
    Make,
    Meson,
    Mix,
}

impl BuildSystemId {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cargo => "Cargo",
            Self::Maven => "Maven",
            Self::Gradle => "Gradle",
            Self::Npm => "npm",
            Self::Yarn => "Yarn",
            Self::Pnpm => "pnpm",
            Self::Bun => "Bun",
            Self::Pip => "pip",
            Self::Poetry => "Poetry",
            Self::Pipenv => "Pipenv",
            Self::GoMod => "go mod",
            Self::DotNet => ".NET",
            Self::Composer => "Composer",
            Self::Bundler => "Bundler",
            Self::CMake => "CMake",
            Self::Make => "Make",
            Self::Meson => "Meson",
            Self::Mix => "Mix",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Cargo" | "cargo" => Some(Self::Cargo),
            "Maven" | "maven" => Some(Self::Maven),
            "Gradle" | "gradle" => Some(Self::Gradle),
            "npm" => Some(Self::Npm),
            "Yarn" | "yarn" => Some(Self::Yarn),
            "pnpm" => Some(Self::Pnpm),
            "Bun" | "bun" => Some(Self::Bun),
            "pip" => Some(Self::Pip),
            "Poetry" | "poetry" => Some(Self::Poetry),
            "Pipenv" | "pipenv" => Some(Self::Pipenv),
            "go mod" | "go-mod" => Some(Self::GoMod),
            ".NET" | "dotnet" => Some(Self::DotNet),
            "Composer" | "composer" => Some(Self::Composer),
            "Bundler" | "bundler" => Some(Self::Bundler),
            "CMake" | "cmake" => Some(Self::CMake),
            "Make" | "make" => Some(Self::Make),
            "Meson" | "meson" => Some(Self::Meson),
            "Mix" | "mix" => Some(Self::Mix),
            _ => None,
        }
    }

    pub fn all_variants() -> &'static [Self] {
        &[
            Self::Cargo,
            Self::Maven,
            Self::Gradle,
            Self::Npm,
            Self::Yarn,
            Self::Pnpm,
            Self::Bun,
            Self::Pip,
            Self::Poetry,
            Self::Pipenv,
            Self::GoMod,
            Self::DotNet,
            Self::Composer,
            Self::Bundler,
            Self::CMake,
            Self::Make,
            Self::Meson,
            Self::Mix,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameworkId {
    #[serde(rename = "spring-boot")]
    SpringBoot,
    Quarkus,
    Micronaut,
    Ktor,
    Express,
    #[serde(rename = "nextjs")]
    NextJs,
    #[serde(rename = "nestjs")]
    NestJs,
    Fastify,
    Django,
    Flask,
    #[serde(rename = "fastapi")]
    FastApi,
    Rails,
    Sinatra,
    #[serde(rename = "actix-web")]
    ActixWeb,
    Axum,
    Gin,
    Echo,
    #[serde(rename = "aspnet-core")]
    AspNetCore,
    Laravel,
    Phoenix,
}

impl FrameworkId {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SpringBoot => "Spring Boot",
            Self::Quarkus => "Quarkus",
            Self::Micronaut => "Micronaut",
            Self::Ktor => "Ktor",
            Self::Express => "Express",
            Self::NextJs => "Next.js",
            Self::NestJs => "NestJS",
            Self::Fastify => "Fastify",
            Self::Django => "Django",
            Self::Flask => "Flask",
            Self::FastApi => "FastAPI",
            Self::Rails => "Rails",
            Self::Sinatra => "Sinatra",
            Self::ActixWeb => "Actix Web",
            Self::Axum => "Axum",
            Self::Gin => "Gin",
            Self::Echo => "Echo",
            Self::AspNetCore => "ASP.NET Core",
            Self::Laravel => "Laravel",
            Self::Phoenix => "Phoenix",
        }
    }

    pub fn all_variants() -> &'static [Self] {
        &[
            Self::SpringBoot,
            Self::Quarkus,
            Self::Micronaut,
            Self::Ktor,
            Self::Express,
            Self::NextJs,
            Self::NestJs,
            Self::Fastify,
            Self::Django,
            Self::Flask,
            Self::FastApi,
            Self::Rails,
            Self::Sinatra,
            Self::ActixWeb,
            Self::Axum,
            Self::Gin,
            Self::Echo,
            Self::AspNetCore,
            Self::Laravel,
            Self::Phoenix,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeId {
    #[serde(rename = "jvm")]
    JVM,
    Node,
    Python,
    Ruby,
    #[serde(rename = "php")]
    PHP,
    #[serde(rename = "dotnet")]
    DotNet,
    #[serde(rename = "beam")]
    BEAM,
    Native,
    #[serde(rename = "llm")]
    LLM,
}

impl std::fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl RuntimeId {
    pub fn name(&self) -> &'static str {
        match self {
            Self::JVM => "JVM",
            Self::Node => "Node",
            Self::Python => "Python",
            Self::Ruby => "Ruby",
            Self::PHP => "PHP",
            Self::DotNet => ".NET",
            Self::BEAM => "BEAM",
            Self::Native => "Native",
            Self::LLM => "LLM",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "JVM" | "java" | "kotlin" => Some(Self::JVM),
            "Node" | "node" => Some(Self::Node),
            "Python" | "python" => Some(Self::Python),
            "Ruby" | "ruby" => Some(Self::Ruby),
            "PHP" | "php" => Some(Self::PHP),
            ".NET" | "dotnet" | "csharp" | "fsharp" => Some(Self::DotNet),
            "BEAM" | "elixir" => Some(Self::BEAM),
            "Native" | "rust" | "c++" | "go" => Some(Self::Native),
            "LLM" => Some(Self::LLM),
            _ => None,
        }
    }

    pub fn all_variants() -> &'static [Self] {
        &[
            Self::JVM,
            Self::Node,
            Self::Python,
            Self::Ruby,
            Self::PHP,
            Self::DotNet,
            Self::BEAM,
            Self::Native,
            Self::LLM,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_id_serialization() {
        assert_eq!(
            serde_json::to_string(&LanguageId::Rust).unwrap(),
            "\"rust\""
        );
        assert_eq!(
            serde_json::to_string(&LanguageId::CSharp).unwrap(),
            "\"csharp\""
        );
        assert_eq!(serde_json::to_string(&LanguageId::Cpp).unwrap(), "\"c++\"");
    }

    #[test]
    fn test_language_id_deserialization() {
        assert_eq!(
            serde_json::from_str::<LanguageId>("\"rust\"").unwrap(),
            LanguageId::Rust
        );
        assert_eq!(
            serde_json::from_str::<LanguageId>("\"csharp\"").unwrap(),
            LanguageId::CSharp
        );
    }

    #[test]
    fn test_build_system_id_serialization() {
        assert_eq!(
            serde_json::to_string(&BuildSystemId::Npm).unwrap(),
            "\"npm\""
        );
        assert_eq!(
            serde_json::to_string(&BuildSystemId::GoMod).unwrap(),
            "\"go-mod\""
        );
    }

    #[test]
    fn test_framework_id_serialization() {
        assert_eq!(
            serde_json::to_string(&FrameworkId::SpringBoot).unwrap(),
            "\"spring-boot\""
        );
        assert_eq!(
            serde_json::to_string(&FrameworkId::NextJs).unwrap(),
            "\"nextjs\""
        );
    }

    #[test]
    fn test_language_id_name() {
        assert_eq!(LanguageId::Rust.name(), "Rust");
        assert_eq!(LanguageId::CSharp.name(), "C#");
        assert_eq!(LanguageId::FSharp.name(), "F#");
        assert_eq!(LanguageId::Cpp.name(), "C++");
    }

    #[test]
    fn test_build_system_id_name() {
        assert_eq!(BuildSystemId::Cargo.name(), "Cargo");
        assert_eq!(BuildSystemId::GoMod.name(), "go mod");
    }

    #[test]
    fn test_framework_id_name() {
        assert_eq!(FrameworkId::SpringBoot.name(), "Spring Boot");
        assert_eq!(FrameworkId::NextJs.name(), "Next.js");
    }
}

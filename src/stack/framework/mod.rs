//! Framework definitions
//!
//! Frameworks are first-class entities that declare compatibility with languages and build systems.
//! Framework detection is deterministic via dependency pattern matching, avoiding LLM calls for
//! major frameworks (Spring Boot, Express, Django, Next.js, Rails, ASP.NET Core).

use crate::stack::buildsystem::BuildTemplate;
use crate::stack::language::Dependency;
use regex::Regex;
use std::path::Path;

/// Dependency pattern for framework detection
#[derive(Debug, Clone)]
pub struct DependencyPattern {
    pub pattern_type: DependencyPatternType,
    pub pattern: String,
    pub confidence: f32,
}

/// Type of dependency pattern matching
#[derive(Debug, Clone)]
pub enum DependencyPatternType {
    /// Maven group:artifact pattern (e.g., "org.springframework.boot:spring-boot-starter-web")
    MavenGroupArtifact,
    /// NPM package name (e.g., "express")
    NpmPackage,
    /// PyPI package name (e.g., "django")
    PypiPackage,
    /// Regex pattern for flexible matching
    Regex,
}

impl DependencyPattern {
    /// Check if a dependency matches this pattern
    pub fn matches(&self, dep: &Dependency) -> bool {
        match self.pattern_type {
            DependencyPatternType::MavenGroupArtifact => {
                // Maven dependencies have format "group:artifact" or "group.subgroup:artifact"
                dep.name.contains(&self.pattern) || dep.name == self.pattern
            }
            DependencyPatternType::NpmPackage | DependencyPatternType::PypiPackage => {
                // Direct package name match
                dep.name == self.pattern
            }
            DependencyPatternType::Regex => {
                // Regex matching
                if let Ok(re) = Regex::new(&self.pattern) {
                    re.is_match(&dep.name)
                } else {
                    false
                }
            }
        }
    }
}

/// Configuration extracted from framework config files
#[derive(Debug, Clone, Default)]
pub struct FrameworkConfig {
    pub port: Option<u16>,
    pub env_vars: Vec<String>,
    pub health_endpoint: Option<String>,
}

/// Framework trait defining framework-specific behavior
pub trait Framework: Send + Sync {
    fn id(&self) -> crate::stack::FrameworkId;

    /// Compatible language names (e.g., ["Java", "Kotlin"] for Spring Boot)
    fn compatible_languages(&self) -> Vec<String>;

    /// Compatible build system names (e.g., ["maven", "gradle"] for Spring Boot)
    fn compatible_build_systems(&self) -> Vec<String>;

    /// Dependency patterns for framework detection
    fn dependency_patterns(&self) -> Vec<DependencyPattern>;

    /// Default ports for this framework (e.g., [8080] for Spring Boot, [3000] for Express)
    fn default_ports(&self) -> Vec<u16>;

    /// Health check endpoints (e.g., ["/actuator/health"] for Spring Boot)
    fn health_endpoints(&self) -> Vec<String>;

    /// Environment variable patterns (regex, description)
    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![]
    }

    /// Config file patterns to search for (e.g., ["application.yml", "settings.py"])
    fn config_files(&self) -> Vec<&str> {
        vec![]
    }

    /// Parse framework-specific config file
    fn parse_config(&self, _file_path: &Path, _content: &str) -> Option<FrameworkConfig> {
        None
    }

    /// Customize build template with framework-specific optimizations
    fn customize_build_template(&self, template: BuildTemplate) -> BuildTemplate {
        template
    }
}

pub mod actix;
pub mod aspnet;
pub mod axum;
pub mod django;
pub mod echo;
pub mod express;
pub mod fastapi;
pub mod fastify;
pub mod flask;
pub mod gin;
pub mod ktor;
pub mod laravel;
pub mod llm;
pub mod micronaut;
pub mod nestjs;
pub mod nextjs;
pub mod phoenix;
pub mod quarkus;
pub mod rails;
pub mod sinatra;
pub mod spring_boot;
pub mod symfony;

pub use actix::ActixFramework;
pub use aspnet::AspNetFramework;
pub use axum::AxumFramework;
pub use django::DjangoFramework;
pub use echo::EchoFramework;
pub use express::ExpressFramework;
pub use fastapi::FastApiFramework;
pub use fastify::FastifyFramework;
pub use flask::FlaskFramework;
pub use gin::GinFramework;
pub use ktor::KtorFramework;
pub use laravel::LaravelFramework;
pub use llm::LLMFramework;
pub use micronaut::MicronautFramework;
pub use nestjs::NestJsFramework;
pub use nextjs::NextJsFramework;
pub use phoenix::PhoenixFramework;
pub use quarkus::QuarkusFramework;
pub use rails::RailsFramework;
pub use sinatra::SinatraFramework;
pub use spring_boot::SpringBootFramework;
pub use symfony::SymfonyFramework;

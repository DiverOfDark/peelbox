// Shared cross-language file format parsers
//
// These parsers extract information from common file formats that are
// language-agnostic (Dockerfile, .env files, YAML/JSON configs, Docker Compose, Kubernetes).
// They are used by multiple extractors to avoid code duplication.

pub mod config;
pub mod docker_compose;
pub mod dockerfile;
pub mod env_file;
pub mod kubernetes;

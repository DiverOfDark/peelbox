//! Environment variable extractor - deterministic extraction of env vars from files and code

use crate::extractors::ServiceContext;
use crate::fs::FileSystem;
use crate::stack::registry::StackRegistry;
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVarInfo {
    pub name: String,
    pub default_value: Option<String>,
    pub source: EnvVarSource,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvVarSource {
    EnvExample,
    EnvTemplate,
    ConfigFile(String),
    CodeReference(String),
}

pub struct EnvVarExtractor<F: FileSystem> {
    fs: F,
    registry: StackRegistry,
}

impl<F: FileSystem> EnvVarExtractor<F> {
    pub fn new(fs: F) -> Self {
        Self {
            fs,
            registry: StackRegistry::with_defaults(None),
        }
    }

    pub fn extract(&self, context: &ServiceContext) -> Vec<EnvVarInfo> {
        let mut env_vars = HashMap::new();

        // Extract from .env.example files (highest priority)
        for env_file in &[".env.example", ".env.template", ".env.sample"] {
            if let Ok(content) = self.fs.read_to_string(&context.path.join(env_file)) {
                self.extract_from_env_file(&content, &mut env_vars, env_file);
            }
        }

        // Extract from config files
        self.extract_from_config_files(context, &mut env_vars);

        // Extract from code references (for variables not found in .env files)
        self.extract_from_code_references(context, &mut env_vars);

        env_vars.into_values().collect()
    }

    fn extract_from_env_file(
        &self,
        content: &str,
        env_vars: &mut HashMap<String, EnvVarInfo>,
        filename: &str,
    ) {
        let env_re = Regex::new(r"(?m)^([A-Z_][A-Z0-9_]*)=(.*)$").expect("valid regex");

        let source = match filename {
            ".env.example" => EnvVarSource::EnvExample,
            ".env.template" => EnvVarSource::EnvTemplate,
            _ => EnvVarSource::EnvExample,
        };

        for cap in env_re.captures_iter(content) {
            if let Some(name_match) = cap.get(1) {
                let name = name_match.as_str().to_string();
                let value = cap.get(2).map(|v| v.as_str().trim().to_string());

                // Determine if required based on value
                let required = value.as_deref() == Some("")
                    || value.as_deref() == Some("REQUIRED")
                    || value.as_deref() == Some("TODO");

                env_vars.entry(name.clone()).or_insert(EnvVarInfo {
                    name,
                    default_value: value
                        .filter(|v| !v.is_empty() && v != "REQUIRED" && v != "TODO"),
                    source: source.clone(),
                    required,
                });
            }
        }
    }

    fn extract_from_config_files(
        &self,
        context: &ServiceContext,
        env_vars: &mut HashMap<String, EnvVarInfo>,
    ) {
        use crate::extractors::parsers;

        parsers::docker_compose::parse_env_vars(&context.path, &self.fs, env_vars);
        parsers::kubernetes::parse_env_vars(&context.path, &self.fs, env_vars);
    }

    fn extract_from_code_references(
        &self,
        context: &ServiceContext,
        env_vars: &mut HashMap<String, EnvVarInfo>,
    ) {
        let lang = match context
            .language
            .as_ref()
            .and_then(|id| self.registry.get_language(id.clone()))
        {
            Some(l) => l,
            None => return,
        };

        let patterns = lang.env_var_patterns();
        let dir_path = &context.path;
        crate::extractors::common::scan_directory_with_language_filter(
            &self.fs,
            dir_path,
            lang,
            |file_path| {
                self.extract_env_vars_from_file(file_path, &patterns, env_vars);
            },
        );
    }

    fn extract_env_vars_from_file(
        &self,
        file_path: &std::path::Path,
        patterns: &[(&str, &str)],
        env_vars: &mut HashMap<String, EnvVarInfo>,
    ) {
        let content = match self.fs.read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        for (pattern, pattern_name) in patterns {
            let re = match Regex::new(pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for cap in re.captures_iter(&content) {
                if let Some(name) = cap.get(1).map(|m| m.as_str().to_string()) {
                    env_vars.entry(name.clone()).or_insert(EnvVarInfo {
                        name,
                        default_value: None,
                        source: EnvVarSource::CodeReference(pattern_name.to_string()),
                        required: false,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::MockFileSystem;
    use std::path::PathBuf;

    #[test]
    fn test_extract_from_env_example() {
        let fs = MockFileSystem::new();
        fs.add_file(
            ".env.example",
            r#"
DATABASE_URL=postgres://localhost:5432/mydb
API_KEY=
PORT=3000
SECRET_KEY=REQUIRED
"#,
        );

        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 4);

        let db_var = env_vars.iter().find(|v| v.name == "DATABASE_URL").unwrap();
        assert_eq!(
            db_var.default_value.as_deref(),
            Some("postgres://localhost:5432/mydb")
        );
        assert!(!db_var.required);

        let api_var = env_vars.iter().find(|v| v.name == "API_KEY").unwrap();
        assert!(api_var.required);
        assert_eq!(api_var.default_value, None);

        let secret_var = env_vars.iter().find(|v| v.name == "SECRET_KEY").unwrap();
        assert!(secret_var.required);
    }

    #[test]
    fn test_extract_from_docker_compose() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "docker-compose.yml",
            r#"
version: '3'
services:
  app:
    environment:
      - DATABASE_URL
      - API_KEY
      - PORT
"#,
        );

        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 3);
        assert!(env_vars.iter().any(|v| v.name == "DATABASE_URL"));
        assert!(env_vars.iter().any(|v| v.name == "API_KEY"));
        assert!(env_vars.iter().any(|v| v.name == "PORT"));
    }

    #[test]
    fn test_extract_from_code_nodejs() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "server.js",
            r#"
const port = process.env.PORT || 3000;
const apiKey = process.env.API_KEY;
const dbUrl = process.env.DATABASE_URL;
"#,
        );

        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::with_detection(
            PathBuf::from("."),
            Some(crate::stack::LanguageId::JavaScript),
            None,
        );
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 3);
        assert!(env_vars.iter().any(|v| v.name == "PORT"));
        assert!(env_vars.iter().any(|v| v.name == "API_KEY"));
        assert!(env_vars.iter().any(|v| v.name == "DATABASE_URL"));
    }

    #[test]
    fn test_extract_from_code_python() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "app.py",
            r#"
import os

port = os.environ.get('PORT', 5000)
api_key = os.getenv('API_KEY')
"#,
        );

        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::with_detection(
            PathBuf::from("."),
            Some(crate::stack::LanguageId::Python),
            None,
        );
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 2);
        assert!(env_vars.iter().any(|v| v.name == "PORT"));
        assert!(env_vars.iter().any(|v| v.name == "API_KEY"));
    }

    #[test]
    fn test_extract_from_code_rust() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "main.rs",
            r#"
use std::env;

fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let api_key = std::env::var("API_KEY").expect("API_KEY must be set");
}
"#,
        );

        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::with_detection(
            PathBuf::from("."),
            Some(crate::stack::LanguageId::Rust),
            None,
        );
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 2);
        assert!(env_vars.iter().any(|v| v.name == "PORT"));
        assert!(env_vars.iter().any(|v| v.name == "API_KEY"));
    }

    #[test]
    fn test_deduplication_prefers_env_example() {
        let fs = MockFileSystem::new();
        fs.add_file(".env.example", "PORT=3000");
        fs.add_file("server.js", "const port = process.env.PORT;");

        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 1);
        let port_var = &env_vars[0];
        assert_eq!(port_var.name, "PORT");
        assert_eq!(port_var.default_value.as_deref(), Some("3000"));
        assert!(matches!(port_var.source, EnvVarSource::EnvExample));
    }

    #[test]
    fn test_no_env_vars_found() {
        let fs = MockFileSystem::new();
        let extractor = EnvVarExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let env_vars = extractor.extract(&context);

        assert_eq!(env_vars.len(), 0);
    }
}

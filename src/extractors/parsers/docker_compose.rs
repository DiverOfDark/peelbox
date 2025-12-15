//! Docker Compose file parsing utilities

use crate::extractors::env_vars::{EnvVarInfo, EnvVarSource};
use crate::fs::FileSystem;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

/// Parse environment variables from docker-compose.yml files
pub fn parse_env_vars<F: FileSystem>(
    service_path: &Path,
    fs: &F,
    env_vars: &mut HashMap<String, EnvVarInfo>,
) {
    if let Ok(content) = fs.read_to_string(&service_path.join("docker-compose.yml")) {
        let env_re = Regex::new(r"(?m)^\s*-\s*([A-Z_][A-Z0-9_]*)").expect("valid regex");

        for cap in env_re.captures_iter(&content) {
            if let Some(name_match) = cap.get(1) {
                let name = name_match.as_str().to_string();
                env_vars.entry(name.clone()).or_insert(EnvVarInfo {
                    name,
                    default_value: None,
                    source: EnvVarSource::ConfigFile("docker-compose.yml".to_string()),
                    required: false,
                });
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
    fn test_parse_env_vars() {
        let mut fs = MockFileSystem::new();
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

        let mut env_vars = HashMap::new();
        parse_env_vars(&PathBuf::from("."), &fs, &mut env_vars);

        assert_eq!(env_vars.len(), 3);
        assert!(env_vars.contains_key("DATABASE_URL"));
        assert!(env_vars.contains_key("API_KEY"));
        assert!(env_vars.contains_key("PORT"));
    }

    #[test]
    fn test_no_docker_compose_file() {
        let fs = MockFileSystem::new();
        let mut env_vars = HashMap::new();

        parse_env_vars(&PathBuf::from("."), &fs, &mut env_vars);
        assert_eq!(env_vars.len(), 0);
    }
}

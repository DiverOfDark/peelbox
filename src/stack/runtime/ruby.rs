use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct RubyRuntime;

impl RubyRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let env_pattern = Regex::new(r#"ENV\[['"]([A-Z_][A-Z0-9_]*)['"]\]"#).unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "rb" || ext == "ru" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        for cap in env_pattern.captures_iter(&content) {
                            if let Some(var) = cap.get(1) {
                                env_vars.insert(var.as_str().to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut vars: Vec<String> = env_vars.into_iter().collect();
        vars.sort();
        vars
    }

    fn extract_ports(&self, files: &[PathBuf]) -> Option<u16> {
        let rack_pattern = Regex::new(r"(?s)Rack::Server.*?Port:\s*(\d+)").unwrap();
        let webrick_pattern = Regex::new(r"(?s)WEBrick.*?:Port\s*=>\s*(\d+)").unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "rb" || ext == "ru" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if let Some(cap) = rack_pattern.captures(&content) {
                            if let Some(port_str) = cap.get(1) {
                                if let Ok(port) = port_str.as_str().parse::<u16>() {
                                    return Some(port);
                                }
                            }
                        }
                        if let Some(cap) = webrick_pattern.captures(&content) {
                            if let Some(port_str) = cap.get(1) {
                                if let Ok(port) = port_str.as_str().parse::<u16>() {
                                    return Some(port);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_native_deps(&self, files: &[PathBuf]) -> Vec<String> {
        let mut deps = HashSet::new();

        for file in files {
            if file.file_name().is_some_and(|n| n == "Gemfile") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if content.contains("pg")
                        || content.contains("mysql2")
                        || content.contains("nokogiri")
                        || content.contains("ffi")
                    {
                        deps.insert("build-base".to_string());
                    }
                }
            }
        }

        let mut result: Vec<String> = deps.into_iter().collect();
        result.sort();
        result
    }
}

impl Runtime for RubyRuntime {
    fn name(&self) -> &str {
        "Ruby"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let env_vars = self.extract_env_vars(files);
        let native_deps = self.extract_native_deps(files);
        let detected_port = self.extract_ports(files);

        let port =
            detected_port.or_else(|| framework.and_then(|f| f.default_ports().first().copied()));
        let health = framework.and_then(|f| {
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint: None,
            port,
            env_vars,
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("3.2");
        format!("ruby:{}-alpine", version)
    }

    fn required_packages(&self) -> Vec<String> {
        vec![]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("ruby {}", entrypoint.display())
    }

    fn runtime_packages(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> Vec<String> {
        let requested = self.detect_version(service_path, manifest_content);
        let available = wolfi_index.get_versions("ruby");

        let version = requested
            .as_deref()
            .and_then(|r| wolfi_index.match_version("ruby", r, &available))
            .or_else(|| wolfi_index.get_latest_version("ruby"))
            .unwrap_or_else(|| "ruby-3.2".to_string());

        vec![version]
    }
}

impl RubyRuntime {
    fn detect_version(&self, service_path: &Path, manifest_content: Option<&str>) -> Option<String> {
        let ruby_version_file = service_path.join(".ruby-version");
        if let Ok(content) = std::fs::read_to_string(&ruby_version_file) {
            if let Some(ver) = self.normalize_version(&content) {
                return Some(ver);
            }
        }

        if let Some(content) = manifest_content {
            if let Some(ver) = self.parse_gemfile_version(content) {
                return Some(ver);
            }
        }

        None
    }

    fn normalize_version(&self, version_str: &str) -> Option<String> {
        let ver = version_str
            .trim()
            .trim_start_matches("ruby")
            .trim()
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");

        if !ver.is_empty() {
            Some(ver)
        } else {
            None
        }
    }

    fn parse_gemfile_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("ruby") && trimmed.contains('"') {
                let parts: Vec<&str> = trimmed.split('"').collect();
                if parts.len() >= 2 {
                    return self.normalize_version(parts[1]);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_ruby_runtime_name() {
        let runtime = RubyRuntime;
        assert_eq!(runtime.name(), "Ruby");
    }

    #[test]
    fn test_ruby_runtime_base_image_default() {
        let runtime = RubyRuntime;
        assert_eq!(runtime.runtime_base_image(None), "ruby:3.2-alpine");
    }

    #[test]
    fn test_ruby_runtime_base_image_versioned() {
        let runtime = RubyRuntime;
        assert_eq!(runtime.runtime_base_image(Some("3.3")), "ruby:3.3-alpine");
    }

    #[test]
    fn test_ruby_required_packages() {
        let runtime = RubyRuntime;
        let packages: Vec<String> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_ruby_start_command() {
        let runtime = RubyRuntime;
        let entrypoint = Path::new("app.rb");
        assert_eq!(runtime.start_command(entrypoint), "ruby app.rb");
    }

    #[test]
    fn test_extract_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let rb_file = temp_dir.path().join("app.rb");
        fs::write(
            &rb_file,
            r#"
db_url = ENV['DATABASE_URL']
api_key = ENV["API_KEY"]
"#,
        )
        .unwrap();

        let runtime = RubyRuntime;
        let files = vec![rb_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL"]);
    }

    #[test]
    fn test_extract_ports_rack() {
        let temp_dir = TempDir::new().unwrap();
        let rb_file = temp_dir.path().join("config.ru");
        let content = r#"
require 'rack'
Rack::Server.start(Port: 9292)
"#;
        fs::write(&rb_file, content).unwrap();

        let runtime = RubyRuntime;
        let files = vec![rb_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(9292));
    }

    #[test]
    fn test_extract_native_deps() {
        let temp_dir = TempDir::new().unwrap();
        let gemfile = temp_dir.path().join("Gemfile");
        fs::write(
            &gemfile,
            r#"
source 'https://rubygems.org'
gem 'pg'
gem 'rails'
"#,
        )
        .unwrap();

        let runtime = RubyRuntime;
        let files = vec![gemfile];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base".to_string()]);
    }
}

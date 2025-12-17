//! Health check extractor - deterministic extraction of health check endpoints

use crate::extractors::ServiceContext;
use crate::fs::FileSystem;
use crate::languages::LanguageRegistry;
use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct HealthCheckInfo {
    pub endpoint: String,
    pub source: HealthCheckSource,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthCheckSource {
    Dockerfile,
    KubernetesManifest(String),
    CodeRoute(String),
    FrameworkDefault(String),
}

pub struct HealthCheckExtractor<F: FileSystem> {
    fs: F,
    registry: LanguageRegistry,
}

impl<F: FileSystem> HealthCheckExtractor<F> {
    pub fn new(fs: F) -> Self {
        Self {
            fs,
            registry: LanguageRegistry::with_defaults(),
        }
    }

    pub fn with_registry(fs: F, registry: LanguageRegistry) -> Self {
        Self { fs, registry }
    }

    pub fn extract(&self, context: &ServiceContext) -> Vec<HealthCheckInfo> {
        let mut health_checks = Vec::new();
        let mut seen = HashSet::new();

        use crate::extractors::parsers;

        health_checks.extend(parsers::dockerfile::parse_healthcheck(
            &context.path,
            &self.fs,
            &mut seen,
        ));

        parsers::kubernetes::parse_health_checks(
            &context.path,
            &self.fs,
            &mut health_checks,
            &mut seen,
        );

        self.extract_from_code_routes(context, &mut health_checks, &mut seen);

        if health_checks.is_empty() {
            self.apply_framework_defaults(context, &mut health_checks, &mut seen);
        }

        health_checks
    }

    fn extract_from_code_routes(
        &self,
        context: &ServiceContext,
        health_checks: &mut Vec<HealthCheckInfo>,
        seen: &mut HashSet<String>,
    ) {
        let lang = match context
            .language
            .as_ref()
            .and_then(|name| self.registry.get_language(name))
        {
            Some(l) => l,
            None => return,
        };

        let patterns = lang.health_check_patterns();
        let dir_path = &context.path;
        crate::extractors::common::scan_directory_with_language_filter(
            &self.fs,
            dir_path,
            lang,
            |file_path| {
                self.extract_health_checks_from_file(file_path, &patterns, health_checks, seen);
            },
        );
    }

    fn extract_health_checks_from_file(
        &self,
        file_path: &std::path::Path,
        patterns: &[(&str, &str)],
        health_checks: &mut Vec<HealthCheckInfo>,
        seen: &mut HashSet<String>,
    ) {
        let content = match self.fs.read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        for (pattern, framework) in patterns {
            let re = match Regex::new(pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for cap in re.captures_iter(&content) {
                if let Some(endpoint) = cap.get(1).map(|m| m.as_str().to_string()) {
                    if seen.insert(endpoint.clone()) {
                        health_checks.push(HealthCheckInfo {
                            endpoint,
                            source: HealthCheckSource::CodeRoute(framework.to_string()),
                            confidence: 0.9,
                        });
                    }
                }
            }
        }
    }

    fn apply_framework_defaults(
        &self,
        context: &ServiceContext,
        health_checks: &mut Vec<HealthCheckInfo>,
        seen: &mut HashSet<String>,
    ) {
        let language = context
            .language
            .as_ref()
            .and_then(|name| self.registry.get_language(name));

        if let Some(lang) = language {
            for (endpoint, framework) in lang.default_health_endpoints() {
                if seen.insert(endpoint.to_string()) {
                    health_checks.push(HealthCheckInfo {
                        endpoint: endpoint.to_string(),
                        source: HealthCheckSource::FrameworkDefault(framework.to_string()),
                        confidence: 0.7,
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
    fn test_extract_from_dockerfile() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "Dockerfile",
            r#"
FROM node:20
HEALTHCHECK CMD curl http://localhost:3000/health
"#,
        );

        let extractor = HealthCheckExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 1);
        assert_eq!(health_checks[0].endpoint, "/health");
        assert!(matches!(
            health_checks[0].source,
            HealthCheckSource::Dockerfile
        ));
    }

    #[test]
    fn test_extract_from_k8s_manifest() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "deployment.yaml",
            r#"
apiVersion: apps/v1
kind: Deployment
spec:
  template:
    spec:
      containers:
      - livenessProbe:
          httpGet:
            path: /healthz
            port: 8080
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
"#,
        );

        let extractor = HealthCheckExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 2);
        assert!(health_checks.iter().any(|h| h.endpoint == "/healthz"));
        assert!(health_checks.iter().any(|h| h.endpoint == "/ready"));
    }

    #[test]
    fn test_extract_from_express_route() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "server.js",
            r#"
const express = require('express');
const app = express();

app.get('/health', (req, res) => {
  res.json({ status: 'ok' });
});
"#,
        );

        let extractor = HealthCheckExtractor::new(fs);
        let context = ServiceContext::with_detection(
            PathBuf::from("."),
            Some("JavaScript".to_string()),
            None,
        );
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 1);
        assert_eq!(health_checks[0].endpoint, "/health");
        assert!(matches!(
            health_checks[0].source,
            HealthCheckSource::CodeRoute(_)
        ));
    }

    #[test]
    fn test_extract_from_spring_boot() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "src/main/java/HealthController.java",
            r#"
@RestController
public class HealthController {
    @GetMapping("/actuator/health")
    public Map<String, String> health() {
        return Map.of("status", "UP");
    }
}
"#,
        );

        let extractor = HealthCheckExtractor::new(fs);
        let context =
            ServiceContext::with_detection(PathBuf::from("."), Some("Java".to_string()), None);
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 1);
        assert_eq!(health_checks[0].endpoint, "/actuator/health");
    }

    #[test]
    fn test_framework_default_spring_boot() {
        let fs = MockFileSystem::new();
        let extractor = HealthCheckExtractor::new(fs);
        let context =
            ServiceContext::with_detection(PathBuf::from("."), Some("Java".to_string()), None);
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 1);
        assert_eq!(health_checks[0].endpoint, "/actuator/health");
        assert!(matches!(
            health_checks[0].source,
            HealthCheckSource::FrameworkDefault(_)
        ));
    }

    #[test]
    fn test_framework_default_nodejs() {
        let fs = MockFileSystem::new();
        let extractor = HealthCheckExtractor::new(fs);
        let context = ServiceContext::with_detection(
            PathBuf::from("."),
            Some("JavaScript".to_string()),
            None,
        );
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 1);
        assert_eq!(health_checks[0].endpoint, "/health");
        assert!(matches!(
            health_checks[0].source,
            HealthCheckSource::FrameworkDefault(_)
        ));
    }

    #[test]
    fn test_deduplication() {
        let fs = MockFileSystem::new();
        fs.add_file(
            "Dockerfile",
            "HEALTHCHECK CMD curl http://localhost:3000/health",
        );
        fs.add_file(
            "deployment.yaml",
            "livenessProbe:\n  httpGet:\n    path: /health",
        );

        let extractor = HealthCheckExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 1);
        assert_eq!(health_checks[0].source, HealthCheckSource::Dockerfile);
    }

    #[test]
    fn test_no_health_checks_found() {
        let fs = MockFileSystem::new();
        let extractor = HealthCheckExtractor::new(fs);
        let context = ServiceContext::new(PathBuf::from("."));
        let health_checks = extractor.extract(&context);

        assert_eq!(health_checks.len(), 0);
    }
}

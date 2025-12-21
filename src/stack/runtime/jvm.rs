use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::stack::framework::Framework;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct JvmRuntime;

impl JvmRuntime {
    fn extract_env_vars(&self, files: &[PathBuf]) -> Vec<String> {
        let mut env_vars = HashSet::new();
        let env_pattern = Regex::new(r#"System\.getenv\("([A-Z_][A-Z0-9_]*)"\)"#).unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "java" || ext == "kt" {
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
        let server_socket_pattern =
            Regex::new(r"ServerSocket\s*\(\s*(\d+)\s*\)").unwrap();
        let jetty_pattern = Regex::new(r"\.setPort\s*\(\s*(\d+)\s*\)").unwrap();

        for file in files {
            if let Some(ext) = file.extension() {
                if ext == "java" || ext == "kt" {
                    if let Ok(content) = std::fs::read_to_string(file) {
                        if let Some(cap) = server_socket_pattern.captures(&content) {
                            if let Some(port_str) = cap.get(1) {
                                if let Ok(port) = port_str.as_str().parse::<u16>() {
                                    return Some(port);
                                }
                            }
                        }
                        if let Some(cap) = jetty_pattern.captures(&content) {
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
            if file.file_name().map_or(false, |n| n == "pom.xml") {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if content.contains("<packaging>so</packaging>")
                        || content.contains("<packaging>jni</packaging>")
                        || content.contains("jna")
                        || content.contains("jni")
                    {
                        deps.insert("build-base".to_string());
                    }
                }
            } else if file.file_name().map_or(false, |n| {
                n == "build.gradle" || n == "build.gradle.kts"
            }) {
                if let Ok(content) = std::fs::read_to_string(file) {
                    if content.contains("jni") || content.contains("jna") {
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

impl Runtime for JvmRuntime {
    fn name(&self) -> &str {
        "JVM"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let env_vars = self.extract_env_vars(files);
        let native_deps = self.extract_native_deps(files);
        let detected_port = self.extract_ports(files);

        let port = detected_port.or_else(|| framework.and_then(|f| f.default_ports().first().copied()));
        let health = framework.and_then(|f| {
            f.health_endpoints().first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        Some(RuntimeConfig {
            entrypoint: Some(self.start_command(Path::new("app.jar"))),
            port,
            env_vars,
            health,
            native_deps,
        })
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        let version = version.unwrap_or("21");
        format!("eclipse-temurin:{}-jre-alpine", version)
    }

    fn required_packages(&self) -> Vec<&str> {
        vec!["ca-certificates"]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("java -jar {}", entrypoint.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_jvm_runtime_name() {
        let runtime = JvmRuntime;
        assert_eq!(runtime.name(), "JVM");
    }

    #[test]
    fn test_jvm_runtime_base_image_default() {
        let runtime = JvmRuntime;
        assert_eq!(
            runtime.runtime_base_image(None),
            "eclipse-temurin:21-jre-alpine"
        );
    }

    #[test]
    fn test_jvm_runtime_base_image_versioned() {
        let runtime = JvmRuntime;
        assert_eq!(
            runtime.runtime_base_image(Some("17")),
            "eclipse-temurin:17-jre-alpine"
        );
    }

    #[test]
    fn test_jvm_required_packages() {
        let runtime = JvmRuntime;
        assert_eq!(runtime.required_packages(), vec!["ca-certificates"]);
    }

    #[test]
    fn test_jvm_start_command() {
        let runtime = JvmRuntime;
        let entrypoint = Path::new("app.jar");
        assert_eq!(runtime.start_command(entrypoint), "java -jar app.jar");
    }

    #[test]
    fn test_extract_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let java_file = temp_dir.path().join("App.java");
        fs::write(
            &java_file,
            r#"
            public class App {
                public static void main(String[] args) {
                    String dbUrl = System.getenv("DATABASE_URL");
                    String apiKey = System.getenv("API_KEY");
                }
            }
            "#,
        )
        .unwrap();

        let runtime = JvmRuntime;
        let files = vec![java_file];
        let env_vars = runtime.extract_env_vars(&files);

        assert_eq!(env_vars, vec!["API_KEY", "DATABASE_URL"]);
    }

    #[test]
    fn test_extract_ports_server_socket() {
        let temp_dir = TempDir::new().unwrap();
        let java_file = temp_dir.path().join("Server.java");
        fs::write(
            &java_file,
            r#"
            import java.net.ServerSocket;
            public class Server {
                public static void main(String[] args) {
                    ServerSocket socket = new ServerSocket(8080);
                }
            }
            "#,
        )
        .unwrap();

        let runtime = JvmRuntime;
        let files = vec![java_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(8080));
    }

    #[test]
    fn test_extract_ports_jetty() {
        let temp_dir = TempDir::new().unwrap();
        let java_file = temp_dir.path().join("Server.java");
        fs::write(
            &java_file,
            r#"
            public class Server {
                public void start() {
                    connector.setPort(9090);
                }
            }
            "#,
        )
        .unwrap();

        let runtime = JvmRuntime;
        let files = vec![java_file];
        let port = runtime.extract_ports(&files);

        assert_eq!(port, Some(9090));
    }

    #[test]
    fn test_extract_native_deps_pom() {
        let temp_dir = TempDir::new().unwrap();
        let pom_file = temp_dir.path().join("pom.xml");
        fs::write(
            &pom_file,
            r#"
            <project>
                <dependencies>
                    <dependency>
                        <groupId>net.java.dev.jna</groupId>
                        <artifactId>jna</artifactId>
                    </dependency>
                </dependencies>
            </project>
            "#,
        )
        .unwrap();

        let runtime = JvmRuntime;
        let files = vec![pom_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base"]);
    }

    #[test]
    fn test_extract_native_deps_gradle() {
        let temp_dir = TempDir::new().unwrap();
        let gradle_file = temp_dir.path().join("build.gradle");
        fs::write(
            &gradle_file,
            r#"
            dependencies {
                implementation 'net.java.dev.jna:jna:5.13.0'
            }
            "#,
        )
        .unwrap();

        let runtime = JvmRuntime;
        let files = vec![gradle_file];
        let deps = runtime.extract_native_deps(&files);

        assert_eq!(deps, vec!["build-base"]);
    }
}

//! Config file parsers that produce `ConfigContribution`.

mod app_config;
mod docker_compose;
mod dockerfile;
mod env_file;
mod kubernetes;
pub(crate) mod mise;
mod procfile;

pub use app_config::AppConfigParser;
pub use docker_compose::DockerComposeParser;
pub use dockerfile::DockerfileParser;
pub use env_file::EnvFileParser;
pub use kubernetes::KubernetesParser;
pub use procfile::ProcfileParser;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ConfigParser;
    use std::path::Path;

    #[test]
    fn test_env_file_parser() {
        let parser = EnvFileParser;
        let content = "# Comment\nPORT=8080\nDATABASE_URL=postgres://localhost\nAPI_KEY=secret";
        let contrib = parser.parse(Path::new(".env"), content).unwrap();
        assert_eq!(contrib.env_vars.len(), 3);
        assert_eq!(contrib.ports, vec![8080]);
    }

    #[test]
    fn test_dockerfile_parser() {
        let parser = DockerfileParser;
        let content =
            "FROM node:18\nEXPOSE 3000\nENV NODE_ENV=production\nCMD [\"node\", \"index.js\"]";
        let contrib = parser.parse(Path::new("Dockerfile"), content).unwrap();
        assert_eq!(contrib.ports, vec![3000]);
        assert_eq!(
            contrib.env_vars.get("NODE_ENV"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_docker_compose_parser() {
        let parser = DockerComposeParser;
        let content = r#"
services:
  web:
    ports:
      - "3000:3000"
    environment:
      - NODE_ENV=production
"#;
        let contrib = parser
            .parse(Path::new("docker-compose.yml"), content)
            .unwrap();
        assert_eq!(contrib.ports, vec![3000]);
    }

    #[test]
    fn test_kubernetes_parser() {
        let parser = KubernetesParser;
        let content = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-app
spec:
  template:
    spec:
      containers:
        - name: my-app
          ports:
            - containerPort: 8080
          env:
            - name: DATABASE_URL
              value: postgres://localhost
            - name: API_KEY
              valueFrom:
                secretKeyRef:
                  name: my-secret
          livenessProbe:
            httpGet:
              path: /healthz
              port: 8080
"#;
        let contrib = parser.parse(Path::new("deployment.yaml"), content).unwrap();
        assert_eq!(contrib.ports, vec![8080]);
        assert!(contrib.env_vars.contains_key("DATABASE_URL"));
        assert!(contrib.env_vars.contains_key("API_KEY"));
        assert_eq!(contrib.health_endpoint, Some("/healthz".to_string()));
    }

    #[test]
    fn test_kubernetes_parser_non_k8s_yaml() {
        let parser = KubernetesParser;
        let content = r#"
name: my-app
port: 8080
database:
  host: localhost
"#;
        let result = parser.parse(Path::new("deployment.yaml"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_app_config_yaml_parser() {
        let parser = AppConfigParser;
        let content = r#"
server:
  port: 8081
spring:
  datasource:
    url: jdbc:postgresql://localhost/mydb
"#;
        let contrib = parser.parse(Path::new("application.yml"), content).unwrap();
        assert_eq!(contrib.ports, vec![8081]);
    }

    #[test]
    fn test_app_config_json_parser() {
        let parser = AppConfigParser;
        let content = r#"
{
  "Urls": "http://0.0.0.0:5000",
  "Port": 5000,
  "ConnectionStrings": {
    "Default": "Server=localhost"
  }
}
"#;
        let contrib = parser
            .parse(Path::new("appsettings.json"), content)
            .unwrap();
        assert_eq!(contrib.ports, vec![5000]);
    }

    #[test]
    fn test_app_config_properties_parser() {
        let parser = AppConfigParser;
        let content = "server.port=9090\nspring.datasource.url=jdbc:postgresql://localhost/mydb\n";
        let contrib = parser
            .parse(Path::new("application.properties"), content)
            .unwrap();
        assert_eq!(contrib.ports, vec![9090]);
    }
}

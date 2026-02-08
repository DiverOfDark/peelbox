//! Config file parsers that produce `ConfigContribution`.

mod docker_compose;
mod dockerfile;
mod env_file;

pub use docker_compose::DockerComposeParser;
pub use dockerfile::DockerfileParser;
pub use env_file::EnvFileParser;

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
        let content = "FROM node:18\nEXPOSE 3000\nENV NODE_ENV=production\nCMD [\"node\", \"index.js\"]";
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
}

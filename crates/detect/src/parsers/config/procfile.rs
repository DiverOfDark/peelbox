use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

pub struct ProcfileParser;

impl ProcfileParser {
    /// Parse a Procfile into a map of process type -> command.
    fn parse_processes(content: &str) -> BTreeMap<String, String> {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .filter_map(|line| {
                let (proc_type, command) = line.split_once(':')?;
                let proc_type = proc_type.trim();
                let command = command.trim();
                if proc_type.is_empty() || command.is_empty() {
                    return None;
                }
                Some((proc_type.to_string(), command.to_string()))
            })
            .collect()
    }

    /// Extract port from a command string by looking for common port patterns.
    fn extract_port(command: &str) -> Option<u16> {
        // Match --bind 0.0.0.0:PORT or --bind :PORT
        let bind_re = regex::Regex::new(r"--bind\s+[\w.]*:(\d+)").ok()?;
        if let Some(cap) = bind_re.captures(command) {
            if let Some(port_match) = cap.get(1) {
                if let Ok(port) = port_match.as_str().parse::<u16>() {
                    return Some(port);
                }
            }
        }

        // Match --port PORT or -p PORT
        let port_flag_re = regex::Regex::new(r"(?:--port|-p)\s+(\d+)").ok()?;
        if let Some(cap) = port_flag_re.captures(command) {
            if let Some(port_match) = cap.get(1) {
                if let Ok(port) = port_match.as_str().parse::<u16>() {
                    return Some(port);
                }
            }
        }

        // Match :PORT at end of host:port patterns like 0.0.0.0:5000
        let host_port_re = regex::Regex::new(r"\d+\.\d+\.\d+\.\d+:(\d+)").ok()?;
        if let Some(cap) = host_port_re.captures(command) {
            if let Some(port_match) = cap.get(1) {
                if let Ok(port) = port_match.as_str().parse::<u16>() {
                    return Some(port);
                }
            }
        }

        None
    }
}

impl ConfigParser for ProcfileParser {
    fn filenames(&self) -> &[&str] {
        &["Procfile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        let processes = Self::parse_processes(content);
        if processes.is_empty() {
            return None;
        }

        // Extract the web process type as the primary runtime command
        let web_command = processes.get("web").cloned();
        if web_command.is_none() {
            return None;
        }

        let command = web_command.as_deref().unwrap();
        let ports = Self::extract_port(command).into_iter().collect();

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars: BTreeMap::new(),
            ports,
            health_endpoint: None,
            runtime_command: web_command,
        })
    }
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(ProcfileParser))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_procfile() {
        let parser = ProcfileParser;
        let content = "web: gunicorn app:app --bind 0.0.0.0:8000";
        let contrib = parser.parse(Path::new("Procfile"), content).unwrap();
        assert_eq!(
            contrib.runtime_command,
            Some("gunicorn app:app --bind 0.0.0.0:8000".to_string())
        );
        assert_eq!(contrib.ports, vec![8000]);
    }

    #[test]
    fn test_parse_multiple_process_types() {
        let parser = ProcfileParser;
        let content = "\
web: gunicorn app:app --bind 0.0.0.0:8000
worker: celery -A tasks worker
release: python manage.py migrate";
        let contrib = parser.parse(Path::new("Procfile"), content).unwrap();
        assert_eq!(
            contrib.runtime_command,
            Some("gunicorn app:app --bind 0.0.0.0:8000".to_string())
        );
        assert_eq!(contrib.ports, vec![8000]);
    }

    #[test]
    fn test_parse_with_comments() {
        let parser = ProcfileParser;
        let content = "\
# This is a comment
web: python -m flask run --port 5000
# Another comment
worker: celery -A tasks worker";
        let contrib = parser.parse(Path::new("Procfile"), content).unwrap();
        assert_eq!(
            contrib.runtime_command,
            Some("python -m flask run --port 5000".to_string())
        );
        assert_eq!(contrib.ports, vec![5000]);
    }

    #[test]
    fn test_parse_no_web_process() {
        let parser = ProcfileParser;
        let content = "worker: celery -A tasks worker";
        let result = parser.parse(Path::new("Procfile"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_empty_procfile() {
        let parser = ProcfileParser;
        let content = "";
        let result = parser.parse(Path::new("Procfile"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_procfile_comments_only() {
        let parser = ProcfileParser;
        let content = "# Just a comment\n# Another comment";
        let result = parser.parse(Path::new("Procfile"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_node_procfile() {
        let parser = ProcfileParser;
        let content = "web: node server.js --port 3000";
        let contrib = parser.parse(Path::new("Procfile"), content).unwrap();
        assert_eq!(
            contrib.runtime_command,
            Some("node server.js --port 3000".to_string())
        );
        assert_eq!(contrib.ports, vec![3000]);
    }

    #[test]
    fn test_parse_procfile_no_port() {
        let parser = ProcfileParser;
        let content = "web: python app.py";
        let contrib = parser.parse(Path::new("Procfile"), content).unwrap();
        assert_eq!(
            contrib.runtime_command,
            Some("python app.py".to_string())
        );
        assert!(contrib.ports.is_empty());
    }

    #[test]
    fn test_parse_processes_helper() {
        let content = "\
web: gunicorn app:app
worker: celery worker
release: python manage.py migrate";
        let processes = ProcfileParser::parse_processes(content);
        assert_eq!(processes.len(), 3);
        assert_eq!(processes.get("web").unwrap(), "gunicorn app:app");
        assert_eq!(processes.get("worker").unwrap(), "celery worker");
        assert_eq!(processes.get("release").unwrap(), "python manage.py migrate");
    }

    #[test]
    fn test_extract_port_bind_flag() {
        assert_eq!(ProcfileParser::extract_port("gunicorn app:app --bind 0.0.0.0:8000"), Some(8000));
    }

    #[test]
    fn test_extract_port_port_flag() {
        assert_eq!(ProcfileParser::extract_port("flask run --port 5000"), Some(5000));
    }

    #[test]
    fn test_extract_port_no_port() {
        assert_eq!(ProcfileParser::extract_port("python app.py"), None);
    }

    #[test]
    fn test_extract_port_ip_port() {
        assert_eq!(ProcfileParser::extract_port("uvicorn app:app --host 0.0.0.0 --port 8080"), Some(8080));
    }
}

use crate::traits::ConfigParser;
use crate::types::ConfigContribution;
use std::collections::BTreeMap;
use std::path::Path;

pub struct EnvFileParser;

impl ConfigParser for EnvFileParser {
    fn filenames(&self) -> &[&str] {
        &[
            ".env",
            ".env.production",
            ".env.local",
            ".env.example",
            ".env.template",
            ".env.sample",
        ]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution> {
        let env_vars: BTreeMap<String, String> = content
            .lines()
            .filter(|line| !line.starts_with('#') && line.contains('='))
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                Some((key.trim().to_string(), value.trim().to_string()))
            })
            .collect();

        if env_vars.is_empty() {
            return None;
        }

        let ports = env_vars
            .iter()
            .filter(|(k, _)| k.contains("PORT"))
            .filter_map(|(_, v)| v.parse::<u16>().ok())
            .collect();

        Some(ConfigContribution {
            path: path.to_path_buf(),
            env_vars,
            ports,
            health_endpoint: None,
        })
    }
}

inventory::submit! {
    crate::registry::ConfigParserEntry(|| Box::new(EnvFileParser))
}

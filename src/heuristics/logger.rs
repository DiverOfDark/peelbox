// Heuristic logging infrastructure for LLM phases
use serde::Serialize;
use std::path::PathBuf;
use tracing::debug;

pub struct HeuristicLogger {
    log_file: Option<PathBuf>,
    enabled: bool,
}

impl HeuristicLogger {
    pub fn new(log_file: Option<PathBuf>) -> Self {
        Self {
            enabled: log_file.is_some(),
            log_file,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            log_file: None,
        }
    }

    pub fn log_phase<I, O>(&self, phase: &str, _input: &I, _output: &O, latency_ms: u64)
    where
        I: Serialize,
        O: Serialize,
    {
        if !self.enabled {
            return;
        }

        debug!(
            "Heuristic log: phase={} latency_ms={} log_file={:?}",
            phase, latency_ms, self.log_file
        );
    }
}

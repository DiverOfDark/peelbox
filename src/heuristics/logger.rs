// Heuristic logging infrastructure for LLM phases
use serde::{Serialize, Serializer};
use serde_json;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

#[derive(Serialize)]
struct HeuristicEntry<I, O>
where
    I: Serialize,
    O: Serialize,
{
    phase: String,
    #[serde(serialize_with = "serialize_as_json")]
    input: I,
    #[serde(serialize_with = "serialize_as_json")]
    output: O,
    latency_ms: u64,
    timestamp: u64,
}

fn serialize_as_json<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    let json_string = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&json_string)
}

pub struct HeuristicLogger {
    writer: Option<Arc<Mutex<BufWriter<File>>>>,
    enabled: bool,
}

impl HeuristicLogger {
    pub fn new(log_file: Option<PathBuf>) -> Self {
        let writer = log_file.and_then(|path| {
            match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(file) => Some(Arc::new(Mutex::new(BufWriter::new(file)))),
                Err(e) => {
                    warn!("Failed to open heuristic log file {:?}: {}", path, e);
                    None
                }
            }
        });

        Self {
            enabled: writer.is_some(),
            writer,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            writer: None,
        }
    }

    pub fn log_phase<I, O>(&self, phase: &str, input: &I, output: &O, latency_ms: u64)
    where
        I: Serialize,
        O: Serialize,
    {
        if !self.enabled {
            return;
        }

        let entry = HeuristicEntry {
            phase: phase.to_string(),
            input,
            output,
            latency_ms,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        if let Some(writer) = &self.writer {
            if let Ok(mut writer) = writer.lock() {
                match serde_json::to_string(&entry) {
                    Ok(json) => {
                        if let Err(e) = writeln!(writer, "{}", json) {
                            warn!("Failed to write heuristic log entry: {}", e);
                        }
                        if let Err(e) = writer.flush() {
                            warn!("Failed to flush heuristic log: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to serialize heuristic entry for phase {}: {}", phase, e);
                    }
                }
            }
        }

        debug!(
            "Heuristic log: phase={} latency_ms={}",
            phase, latency_ms
        );
    }
}

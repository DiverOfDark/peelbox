//! Logging-based progress handler

use super::{ProgressEvent, ProgressHandler};
use tracing::{debug, info, warn};

/// Handler that logs progress events using tracing
#[derive(Debug, Default, Clone, Copy)]
pub struct LoggingHandler;

impl ProgressHandler for LoggingHandler {
    fn on_progress(&self, event: &ProgressEvent) {
        match event {
            ProgressEvent::Started { repo_path } => {
                info!(repo = %repo_path, "Starting detection");
            }
            ProgressEvent::BootstrapComplete {
                languages_detected,
                scan_time,
            } => {
                info!(
                    languages = languages_detected,
                    scan_time_ms = scan_time.as_millis(),
                    "Bootstrap scan complete"
                );
            }
            ProgressEvent::LlmRequestStarted { iteration } => {
                debug!(iteration, "Sending request to LLM");
            }
            ProgressEvent::LlmResponseReceived {
                iteration,
                has_tool_call,
                response_time,
            } => {
                debug!(
                    iteration,
                    has_tool_call,
                    response_time_ms = response_time.as_millis(),
                    "Received LLM response"
                );
            }
            ProgressEvent::ToolExecutionStarted {
                tool_name,
                iteration,
            } => {
                debug!(tool = %tool_name, iteration, "Executing tool");
            }
            ProgressEvent::ToolExecutionComplete {
                tool_name,
                iteration,
                execution_time,
                success,
            } => {
                if *success {
                    debug!(
                        tool = %tool_name,
                        iteration,
                        execution_time_ms = execution_time.as_millis(),
                        "Tool execution complete"
                    );
                } else {
                    warn!(
                        tool = %tool_name,
                        iteration,
                        execution_time_ms = execution_time.as_millis(),
                        "Tool execution failed"
                    );
                }
            }
            ProgressEvent::ValidationStarted => {
                debug!("Starting validation");
            }
            ProgressEvent::ValidationComplete { warnings, errors } => {
                if *errors > 0 {
                    warn!(warnings, errors, "Validation complete with errors");
                } else if *warnings > 0 {
                    info!(warnings, "Validation complete with warnings");
                } else {
                    debug!("Validation complete");
                }
            }
            ProgressEvent::PhaseStarted { phase } => {
                info!(phase = %phase, "Starting phase");
            }
            ProgressEvent::PhaseComplete { phase, duration } => {
                info!(
                    phase = %phase,
                    duration_ms = duration.as_millis(),
                    "Phase complete"
                );
            }
            ProgressEvent::ServiceAnalysisStarted {
                service_path,
                index,
                total,
            } => {
                info!(
                    service = %service_path,
                    progress = format!("{}/{}", index, total),
                    "Analyzing service"
                );
            }
            ProgressEvent::ServiceAnalysisComplete {
                service_path,
                index,
                total,
                duration,
            } => {
                info!(
                    service = %service_path,
                    progress = format!("{}/{}", index, total),
                    duration_ms = duration.as_millis(),
                    "Service analysis complete"
                );
            }
            ProgressEvent::Completed {
                total_iterations,
                total_time,
            } => {
                info!(
                    iterations = total_iterations,
                    total_time_ms = total_time.as_millis(),
                    "Detection complete"
                );
            }
            ProgressEvent::Failed { error } => {
                warn!(error = %error, "Detection failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_logging_handler_creation() {
        let handler = LoggingHandler;
        // Should not panic
        handler.on_progress(&ProgressEvent::Started {
            repo_path: "/test".to_string(),
        });
    }

    #[test]
    fn test_logging_all_events() {
        let handler = LoggingHandler;

        // Test all event types to ensure they don't panic
        let events = vec![
            ProgressEvent::Started {
                repo_path: "/test".to_string(),
            },
            ProgressEvent::BootstrapComplete {
                languages_detected: 2,
                scan_time: Duration::from_millis(50),
            },
            ProgressEvent::LlmRequestStarted { iteration: 1 },
            ProgressEvent::LlmResponseReceived {
                iteration: 1,
                has_tool_call: true,
                response_time: Duration::from_millis(100),
            },
            ProgressEvent::ToolExecutionStarted {
                tool_name: "read_file".to_string(),
                iteration: 1,
            },
            ProgressEvent::ToolExecutionComplete {
                tool_name: "read_file".to_string(),
                iteration: 1,
                execution_time: Duration::from_millis(10),
                success: true,
            },
            ProgressEvent::ToolExecutionComplete {
                tool_name: "read_file".to_string(),
                iteration: 1,
                execution_time: Duration::from_millis(10),
                success: false,
            },
            ProgressEvent::ValidationStarted,
            ProgressEvent::ValidationComplete {
                warnings: 0,
                errors: 0,
            },
            ProgressEvent::ValidationComplete {
                warnings: 1,
                errors: 0,
            },
            ProgressEvent::ValidationComplete {
                warnings: 0,
                errors: 1,
            },
            ProgressEvent::Completed {
                total_iterations: 3,
                total_time: Duration::from_secs(5),
            },
            ProgressEvent::Failed {
                error: "Test error".to_string(),
            },
        ];

        for event in events {
            handler.on_progress(&event);
        }
    }
}

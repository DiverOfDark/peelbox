//! Progress handler trait and events

use std::time::Duration;

/// Events emitted during detection progress
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Detection started
    Started { repo_path: String },

    /// Bootstrap scan completed
    BootstrapComplete {
        languages_detected: usize,
        scan_time: Duration,
    },

    /// LLM request started
    LlmRequestStarted { iteration: usize },

    /// LLM response received
    LlmResponseReceived {
        iteration: usize,
        tool_calls: usize,
        response_time: Duration,
    },

    /// Tool execution started
    ToolExecutionStarted { tool_name: String, iteration: usize },

    /// Tool execution completed
    ToolExecutionComplete {
        tool_name: String,
        iteration: usize,
        execution_time: Duration,
        success: bool,
    },

    /// Validation started
    ValidationStarted,

    /// Validation completed
    ValidationComplete { warnings: usize, errors: usize },

    /// Detection completed successfully
    Completed {
        total_iterations: usize,
        total_time: Duration,
    },

    /// Detection failed
    Failed { error: String },
}

/// Trait for handling progress events during detection
pub trait ProgressHandler: Send + Sync {
    /// Called when a progress event occurs
    fn on_progress(&self, event: &ProgressEvent);
}

/// No-op handler that ignores all events
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpHandler;

impl ProgressHandler for NoOpHandler {
    fn on_progress(&self, _event: &ProgressEvent) {
        // Intentionally empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    impl ProgressHandler for CountingHandler {
        fn on_progress(&self, _event: &ProgressEvent) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_noop_handler() {
        let handler = NoOpHandler;
        handler.on_progress(&ProgressEvent::Started {
            repo_path: "/test".to_string(),
        });
        // Should not panic or do anything
    }

    #[test]
    fn test_progress_events() {
        let count = Arc::new(AtomicUsize::new(0));
        let handler = CountingHandler {
            count: count.clone(),
        };

        handler.on_progress(&ProgressEvent::Started {
            repo_path: "/test".to_string(),
        });
        handler.on_progress(&ProgressEvent::BootstrapComplete {
            languages_detected: 2,
            scan_time: Duration::from_millis(50),
        });
        handler.on_progress(&ProgressEvent::Completed {
            total_iterations: 3,
            total_time: Duration::from_secs(5),
        });

        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_event_debug() {
        let event = ProgressEvent::LlmRequestStarted { iteration: 1 };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("LlmRequestStarted"));
        assert!(debug_str.contains("iteration: 1"));
    }
}

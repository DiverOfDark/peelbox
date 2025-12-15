use std::time::Duration;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Started {
        repo_path: String,
    },
    BootstrapComplete {
        languages_detected: usize,
        scan_time: Duration,
    },
    LlmRequestStarted {
        iteration: usize,
    },
    LlmResponseReceived {
        iteration: usize,
        has_tool_call: bool,
        response_time: Duration,
    },
    ToolExecutionStarted {
        tool_name: String,
        iteration: usize,
    },
    ToolExecutionComplete {
        tool_name: String,
        iteration: usize,
        execution_time: Duration,
        success: bool,
    },
    ValidationStarted,
    ValidationComplete {
        warnings: usize,
        errors: usize,
    },
    PhaseStarted {
        phase: String,
    },
    PhaseComplete {
        phase: String,
        duration: Duration,
    },
    ServiceAnalysisStarted {
        service_path: String,
        index: usize,
        total: usize,
    },
    ServiceAnalysisComplete {
        service_path: String,
        index: usize,
        total: usize,
        duration: Duration,
    },
    Completed {
        total_iterations: usize,
        total_time: Duration,
    },
    Failed {
        error: String,
    },
}

pub trait ProgressHandler: Send + Sync {
    fn on_progress(&self, event: &ProgressEvent);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpHandler;

impl ProgressHandler for NoOpHandler {
    fn on_progress(&self, _event: &ProgressEvent) {}
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

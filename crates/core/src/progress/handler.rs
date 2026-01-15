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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_debug() {
        let event = ProgressEvent::LlmRequestStarted { iteration: 1 };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("LlmRequestStarted"));
        assert!(debug_str.contains("iteration: 1"));
    }
}

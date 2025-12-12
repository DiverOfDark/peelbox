//! Pipeline configuration

use std::time::Duration;

/// Configuration for the analysis pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Maximum number of LLM conversation iterations
    pub max_iterations: usize,

    /// Timeout for the entire analysis
    pub timeout: Duration,

    /// Maximum file size to read in bytes
    pub max_file_size: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            timeout: Duration::from_secs(300),
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

impl PipelineConfig {
    /// Create a new pipeline configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set maximum file size
    pub fn with_max_file_size(mut self, max_file_size: u64) -> Self {
        self.max_file_size = max_file_size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PipelineConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert_eq!(config.max_file_size, 1024 * 1024);
    }

    #[test]
    fn test_builder_pattern() {
        let config = PipelineConfig::new()
            .with_max_iterations(20)
            .with_timeout(Duration::from_secs(600))
            .with_max_file_size(2 * 1024 * 1024);

        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.timeout, Duration::from_secs(600));
        assert_eq!(config.max_file_size, 2 * 1024 * 1024);
    }
}

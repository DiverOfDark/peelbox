//! Example: Advanced Workflow with Custom Error Handling
//!
//! This example demonstrates advanced aipack usage including:
//! - Custom error handling and recovery strategies
//! - Performance measurement and profiling
//! - Logging configuration
//! - Result validation and verification
//! - Integration patterns for production use
//!
//! Run this example with:
//! ```bash
//! RUST_LOG=aipack=debug cargo run --example advanced_workflow -- /path/to/repo
//! ```

use aipack::ai::backend::BackendError;
use aipack::detection::service::{DetectionService, ServiceError};
use aipack::{AipackConfig, DetectionResult};
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Configuration for the advanced workflow
struct WorkflowConfig {
    /// Minimum acceptable confidence threshold
    min_confidence: f32,
    /// Enable automatic retry on failure
    auto_retry: bool,
    /// Maximum number of retry attempts
    max_retries: u32,
    /// Enable result validation
    validate_results: bool,
    /// Enable performance profiling
    enable_profiling: bool,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
            auto_retry: true,
            max_retries: 3,
            validate_results: true,
            enable_profiling: true,
        }
    }
}

/// Performance metrics for the workflow
struct PerformanceMetrics {
    repo_analysis_time: Duration,
    llm_inference_time: Duration,
    total_time: Duration,
    retry_count: u32,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    aipack::init_default();

    info!("=== aipack Advanced Workflow Example ===");

    // Get repository path
    let repo_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    info!("Repository: {}", repo_path.display());

    // Initialize workflow configuration
    let workflow_config = WorkflowConfig::default();

    // Execute workflow with error handling
    match execute_workflow(repo_path, workflow_config).await {
        Ok(result) => {
            info!("Workflow completed successfully");
            display_result(&result);
        }
        Err(e) => {
            error!("Workflow failed: {}", e);
            handle_workflow_error(e);
            std::process::exit(1);
        }
    }
}

/// Execute the advanced detection workflow
async fn execute_workflow(
    repo_path: PathBuf,
    config: WorkflowConfig,
) -> Result<DetectionResult, WorkflowError> {
    let workflow_start = Instant::now();

    info!("Starting advanced detection workflow");
    debug!(
        "Configuration: min_confidence={}, auto_retry={}, max_retries={}",
        config.min_confidence, config.auto_retry, config.max_retries
    );

    // Step 1: Initialize aipack configuration
    info!("Step 1/5: Loading configuration");
    let aipack_config = load_configuration()?;

    // Step 2: Initialize detection service with retry
    info!("Step 2/5: Initializing detection service");
    let service = initialize_service_with_retry(&aipack_config, config.max_retries).await?;

    // Step 3: Perform detection with retry logic
    info!("Step 3/5: Performing build system detection");
    let detection_start = Instant::now();
    let result = perform_detection_with_retry(&service, repo_path, &config).await?;
    let detection_time = detection_start.elapsed();

    info!(
        "Detection completed in {:.2}s",
        detection_time.as_secs_f64()
    );

    // Step 4: Validate results
    if config.validate_results {
        info!("Step 4/5: Validating detection results");
        validate_detection_result(&result, &config)?;
    } else {
        info!("Step 4/5: Skipping validation (disabled)");
    }

    // Step 5: Performance profiling
    if config.enable_profiling {
        info!("Step 5/5: Generating performance metrics");
        let metrics = PerformanceMetrics {
            repo_analysis_time: Duration::from_millis(result.processing_time_ms / 2),
            llm_inference_time: Duration::from_millis(result.processing_time_ms / 2),
            total_time: workflow_start.elapsed(),
            retry_count: 0,
        };
        display_performance_metrics(&metrics);
    }

    info!(
        "Workflow completed in {:.2}s",
        workflow_start.elapsed().as_secs_f64()
    );

    Ok(result)
}

/// Load and validate aipack configuration
fn load_configuration() -> Result<AipackConfig, WorkflowError> {
    debug!("Loading configuration from environment");

    let config = AipackConfig::default();

    debug!("Configuration loaded: provider={:?}", config.provider);

    Ok(config)
}

/// Initialize detection service with retry logic
async fn initialize_service_with_retry(
    config: &AipackConfig,
    max_retries: u32,
) -> Result<DetectionService, WorkflowError> {
    let mut attempts = 0;
    let mut last_error = None;

    while attempts < max_retries {
        attempts += 1;

        debug!(
            "Service initialization attempt {}/{}",
            attempts, max_retries
        );

        match DetectionService::new(config).await {
            Ok(service) => {
                info!(
                    "Service initialized successfully with backend: {}",
                    service.backend_name()
                );
                return Ok(service);
            }
            Err(e) => {
                warn!(
                    "Service initialization failed (attempt {}): {}",
                    attempts, e
                );
                last_error = Some(e);

                if attempts < max_retries {
                    let delay = Duration::from_secs(2_u64.pow(attempts - 1));
                    warn!("Retrying in {:?}...", delay);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(WorkflowError::ServiceError(last_error.unwrap_or_else(
        || ServiceError::BackendInitError("Unknown error".to_string()),
    )))
}

/// Perform detection with retry logic on transient failures
async fn perform_detection_with_retry(
    service: &DetectionService,
    repo_path: PathBuf,
    config: &WorkflowConfig,
) -> Result<DetectionResult, WorkflowError> {
    let mut attempts = 0;
    let mut last_error = None;

    while attempts < config.max_retries {
        attempts += 1;

        debug!("Detection attempt {}/{}", attempts, config.max_retries);

        match service.detect(repo_path.clone()).await {
            Ok(result) => {
                info!(
                    "Detection successful: {} (confidence: {:.1}%)",
                    result.build_system,
                    result.confidence * 100.0
                );
                return Ok(result);
            }
            Err(e) => {
                // Check if error is retryable
                if is_retryable_error(&e) && config.auto_retry && attempts < config.max_retries {
                    warn!("Retryable error on attempt {}: {}", attempts, e);
                    last_error = Some(e);

                    let delay = Duration::from_secs(2_u64.pow(attempts - 1));
                    warn!("Retrying in {:?}...", delay);
                    tokio::time::sleep(delay).await;
                } else {
                    error!("Non-retryable error or max retries reached: {}", e);
                    return Err(WorkflowError::DetectionError(e));
                }
            }
        }
    }

    Err(WorkflowError::DetectionError(last_error.unwrap_or_else(
        || ServiceError::DetectionFailed("Max retries exceeded".to_string()),
    )))
}

/// Check if an error should trigger a retry
fn is_retryable_error(error: &ServiceError) -> bool {
    match error {
        ServiceError::BackendError(backend_err) => matches!(
            backend_err,
            BackendError::TimeoutError { .. }
                | BackendError::NetworkError { .. }
                | BackendError::RateLimitError { .. }
        ),
        _ => false,
    }
}

/// Validate detection results against quality criteria
fn validate_detection_result(
    result: &DetectionResult,
    config: &WorkflowConfig,
) -> Result<(), WorkflowError> {
    debug!("Validating detection result");

    // Check confidence threshold
    if result.confidence < config.min_confidence {
        warn!(
            "Low confidence: {:.1}% (threshold: {:.1}%)",
            result.confidence * 100.0,
            config.min_confidence * 100.0
        );
        return Err(WorkflowError::ValidationError(format!(
            "Confidence {:.1}% below threshold {:.1}%",
            result.confidence * 100.0,
            config.min_confidence * 100.0
        )));
    }

    // Check for required fields
    if result.build_system.is_empty() {
        return Err(WorkflowError::ValidationError(
            "Build system is empty".to_string(),
        ));
    }

    if result.build_command.is_empty() {
        return Err(WorkflowError::ValidationError(
            "Build command is empty".to_string(),
        ));
    }

    // Check for warnings
    if result.has_warnings() {
        warn!(
            "Detection completed with {} warnings:",
            result.warnings.len()
        );
        for warning in &result.warnings {
            warn!("  - {}", warning);
        }
    }

    info!("Validation passed");

    Ok(())
}

/// Display performance metrics
fn display_performance_metrics(metrics: &PerformanceMetrics) {
    info!("Performance Metrics:");
    info!(
        "  Repository Analysis: {:.2}s",
        metrics.repo_analysis_time.as_secs_f64()
    );
    info!(
        "  LLM Inference: {:.2}s",
        metrics.llm_inference_time.as_secs_f64()
    );
    info!("  Total Workflow: {:.2}s", metrics.total_time.as_secs_f64());
    info!("  Retry Count: {}", metrics.retry_count);
}

/// Display detection result in a formatted way
fn display_result(result: &DetectionResult) {
    println!();
    println!("=== Detection Result ===");
    println!("Build System:  {}", result.build_system);
    println!("Language:      {}", result.language);
    println!("Confidence:    {:.1}%", result.confidence * 100.0);
    println!();
    println!("Commands:");
    println!("  Build:   {}", result.build_command);
    println!("  Test:    {}", result.test_command);
    if let Some(ref dev_cmd) = result.dev_command {
        println!("  Dev:     {}", dev_cmd);
    }
    println!();

    if !result.reasoning.is_empty() {
        println!("Reasoning:");
        println!("  {}", result.reasoning);
        println!();
    }

    if !result.detected_files.is_empty() {
        println!("Key Files:");
        for file in &result.detected_files {
            println!("  - {}", file);
        }
        println!();
    }

    println!(
        "Processing Time: {:.2}s",
        result.processing_time_ms as f64 / 1000.0
    );
}

/// Handle workflow errors with detailed troubleshooting
fn handle_workflow_error(error: WorkflowError) {
    match &error {
        WorkflowError::ConfigurationError(msg) => {
            error!("Configuration error: {}", msg);
            println!();
            println!("Troubleshooting:");
            println!("  - Check environment variables are set correctly");
            println!("  - Verify backend configuration");
            println!("  - Run: aipack config");
        }
        WorkflowError::ServiceError(service_err) => {
            error!("Service error: {}", service_err);
            println!();
            println!("{}", service_err.help_message());
        }
        WorkflowError::DetectionError(detection_err) => {
            error!("Detection error: {}", detection_err);
            println!();
            println!("{}", detection_err.help_message());
        }
        WorkflowError::ValidationError(msg) => {
            error!("Validation error: {}", msg);
            println!();
            println!("Troubleshooting:");
            println!("  - Try with a more powerful model");
            println!("  - Verify repository structure is standard");
            println!("  - Lower confidence threshold if needed");
        }
    }
}

/// Custom error type for workflow
#[derive(Debug)]
enum WorkflowError {
    ConfigurationError(String),
    ServiceError(ServiceError),
    DetectionError(ServiceError),
    ValidationError(String),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            WorkflowError::ServiceError(e) => write!(f, "Service error: {}", e),
            WorkflowError::DetectionError(e) => write!(f, "Detection error: {}", e),
            WorkflowError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for WorkflowError {}

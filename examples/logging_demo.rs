//! Logging demonstration for aipack
//!
//! This example shows how to initialize and use structured logging.

use tracing::{debug, error, info, warn};

fn main() {
    println!("=== Aipack Logging Demo ===\n");

    // Initialize logging with default configuration (INFO level)
    aipack::init_default();

    // Basic logging at different levels
    info!("Application started successfully");
    info!(version = env!("CARGO_PKG_VERSION"), "Aipack version");

    // Structured logging with key-value pairs
    let repo_name = "example-repo";
    let file_count = 42;
    debug!(repo = repo_name, files = file_count, "Analyzing repository");

    // Warning logs
    warn!(
        backend = "ollama",
        endpoint = "http://localhost:11434",
        "Backend connection slow"
    );

    // Error logs
    let error_msg = "Configuration validation failed";
    error!(error = error_msg, "Critical error occurred");

    // Demonstrate different initialization methods
    println!("\nOther initialization methods:");
    println!("  aipack::init_default() - Default INFO level");
    println!("  aipack::init_from_env() - Read from AIPACK_LOG_LEVEL");
    println!("  aipack::with_level(\"debug\") - Specific level");
    println!("\nEnvironment variables:");
    println!("  AIPACK_LOG_LEVEL - Set log level (trace, debug, info, warn, error)");
    println!("  AIPACK_LOG_JSON - Use JSON output (true/false)");
    println!("  RUST_LOG - Standard Rust log filtering");
}

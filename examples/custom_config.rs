//! Example: Using Custom Configuration
//!
//! This example demonstrates how to:
//! - Use different LLM models
//! - Configure environment variables
//! - Compare detection results
//!
//! Run this example with:
//! ```bash
//! cargo run --example custom_config -- /path/to/repo
//! ```

use aipack::{AipackConfig, DetectionService};
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Initialize logging
    aipack::init_default();

    println!("=== aipack Custom Configuration Example ===");
    println!();

    // Get repository path
    let repo_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    println!("Repository: {}", repo_path.display());
    println!();

    // Example 1: Using default configuration
    println!("Example 1: Default Configuration");
    println!("---------------------------------");

    let config = AipackConfig::default();
    println!("Provider: {:?}", config.provider);
    println!("Model: {}", config.model);
    println!();

    match DetectionService::new(&config).await {
        Ok(service) => {
            println!("Service initialized successfully");
            println!("Using backend: {}", service.backend_name());

            match service.detect(repo_path.clone()).await {
                Ok(result) => {
                    println!("✓ Detection successful!");
                    println!("  Build System: {}", result.build_system);
                    println!("  Language: {}", result.language);
                    println!("  Confidence: {:.1}%", result.confidence * 100.0);
                    println!("  Build Command: {}", result.build_command);
                }
                Err(e) => {
                    eprintln!("✗ Detection failed: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to initialize service: {}", e);
            eprintln!("{}", e.help_message());
        }
    }

    println!();

    // Example 2: Using environment variables
    println!("Example 2: Environment Variable Configuration");
    println!("----------------------------------------------");
    println!("You can customize behavior with environment variables:");
    println!();
    println!("  AIPACK_BACKEND=ollama                # Backend selection");
    println!("  AIPACK_OLLAMA_MODEL=qwen:14b         # Use larger model");
    println!("  AIPACK_OLLAMA_ENDPOINT=http://...:11434  # Custom endpoint");
    println!("  RUST_LOG=aipack=debug                # Enable debug logging");
    println!();
    println!("Example:");
    println!("  AIPACK_OLLAMA_MODEL=qwen:14b cargo run --example custom_config");
    println!();

    // Show current environment variable configuration
    println!("Current environment:");
    if let Ok(backend) = env::var("AIPACK_BACKEND") {
        println!("  AIPACK_BACKEND: {}", backend);
    }
    if let Ok(model) = env::var("AIPACK_OLLAMA_MODEL") {
        println!("  AIPACK_OLLAMA_MODEL: {}", model);
    }
    if let Ok(endpoint) = env::var("AIPACK_OLLAMA_ENDPOINT") {
        println!("  AIPACK_OLLAMA_ENDPOINT: {}", endpoint);
    }
    if let Ok(log) = env::var("RUST_LOG") {
        println!("  RUST_LOG: {}", log);
    }

    println!();
    println!("For more advanced configuration options, see:");
    println!("  - docs/CONFIGURATION_GUIDE.md");
    println!("  - docs/EXAMPLES.md");
}

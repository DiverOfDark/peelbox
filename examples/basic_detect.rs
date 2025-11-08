//! Example: Basic Build System Detection
//!
//! This example demonstrates the simplest usage of aipack to detect
//! a repository's build system and get build commands.
//!
//! Run this example with:
//! ```bash
//! # Detect current directory
//! cargo run --example basic_detect
//!
//! # Detect specific repository
//! cargo run --example basic_detect -- /path/to/repo
//! ```
//!
//! Prerequisites:
//! - Ollama running locally (or MISTRAL_API_KEY set)
//! - qwen2.5-coder:7b model pulled in Ollama

use aipack::{AipackConfig, DetectionService};
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Initialize logging for better visibility
    aipack::init_default();

    // Get repository path from command line or use current directory
    let repo_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    println!("=== aipack Basic Detection Example ===");
    println!("Repository: {}", repo_path.display());
    println!();

    // Create default configuration
    // This will use environment variables or fall back to sensible defaults
    let config = AipackConfig::default();

    println!("Provider: {:?}", config.provider);
    println!("Model: {}", config.model);
    println!();

    // Create detection service
    let service = match DetectionService::new(&config).await {
        Ok(svc) => svc,
        Err(e) => {
            eprintln!("Error: Failed to initialize detection service");
            eprintln!("{}", e);
            eprintln!();
            eprintln!("{}", e.help_message());
            std::process::exit(1);
        }
    };

    println!("Detecting build system...");
    println!();

    // Perform detection
    match service.detect(repo_path).await {
        Ok(result) => {
            // Display results
            println!("✓ Detection completed successfully!");
            println!();
            println!("Build System: {}", result.build_system);
            println!("Language:     {}", result.language);
            println!("Confidence:   {:.1}%", result.confidence * 100.0);
            println!();
            println!("Commands:");
            println!("  Build:  {}", result.build_command);
            println!("  Test:   {}", result.test_command);
            println!();

            if !result.reasoning.is_empty() {
                println!("Reasoning:");
                println!("  {}", result.reasoning);
                println!();
            }

            if !result.detected_files.is_empty() {
                println!("Detected Files:");
                for file in &result.detected_files {
                    println!("  - {}", file);
                }
                println!();
            }

            if result.has_warnings() {
                println!("⚠ Warnings:");
                for warning in &result.warnings {
                    println!("  - {}", warning);
                }
                println!();
            }

            println!(
                "Processing Time: {:.2}s",
                result.processing_time_ms as f64 / 1000.0
            );

            // Check confidence and provide feedback
            if result.confidence < 0.7 {
                println!();
                println!("⚠ Note: Low confidence detection");
                println!("Consider:");
                println!("  - Ensuring standard build configuration files exist");
                println!("  - Using a more powerful model (qwen:14b)");
                println!("  - Verifying the suggested commands before use");
            } else if result.confidence > 0.9 {
                println!();
                println!("✓ High confidence - commands should be reliable!");
            }
        }
        Err(e) => {
            eprintln!("✗ Detection failed");
            eprintln!();
            eprintln!("{}", e.help_message());
            std::process::exit(1);
        }
    }
}

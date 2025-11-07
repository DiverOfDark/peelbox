//! Basic configuration example for aipack
//!
//! This example demonstrates how to load, validate, and use the aipack configuration.

use aipack::{AipackConfig, ConfigError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging first
    aipack::init_default();

    println!("=== Aipack Configuration Example ===\n");

    // Load configuration from environment with defaults
    let config = AipackConfig::default();

    // Display current configuration
    println!("{}", config);

    // Validate the configuration
    match config.validate() {
        Ok(()) => println!("✓ Configuration is valid\n"),
        Err(e) => {
            eprintln!("✗ Configuration error: {}\n", e);
            return Err(Box::new(e));
        }
    }

    // Check which backends are available
    println!("Backend availability:");
    println!("  Ollama available: {}", config.is_ollama_available());
    println!("  Mistral API key set: {}\n", config.has_mistral_key());

    // Try to get the selected backend configuration
    match config.selected_backend_config() {
        Ok(backend_config) => {
            println!("✓ Selected backend: {}", backend_config.model_name());
            println!("  Timeout: {}s", backend_config.timeout_seconds());
        }
        Err(ConfigError::ValidationFailed(msg)) => {
            println!("✗ Backend selection failed: {}", msg);
            println!("\nTo use aipack, either:");
            println!("  1. Start Ollama: ollama serve");
            println!("  2. Set MISTRAL_API_KEY environment variable");
        }
        Err(e) => {
            return Err(Box::new(e));
        }
    }

    // If caching is enabled, show cache path
    if config.cache_enabled {
        let cache_path = config.cache_path("example-repo");
        println!("\nCache enabled:");
        println!("  Path: {}", cache_path.display());
    }

    Ok(())
}

//! Basic configuration example for aipack
//!
//! This example demonstrates how to load, validate, and use the aipack configuration.

use aipack::AipackConfig;

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

    // Display configured provider
    println!("Configured provider: {:?}\n", config.provider);

    // If caching is enabled, show cache path
    if config.cache_enabled {
        let cache_path = config.cache_path("example-repo");
        println!("Cache enabled:");
        println!("  Path: {}", cache_path.display());
    } else {
        println!("Cache disabled");
    }

    Ok(())
}

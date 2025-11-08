//! Complete example of using aipack with Ollama for build system detection
//!
//! This example demonstrates the full workflow of detecting a build system
//! using the Ollama backend. It shows:
//!
//! - Configuration setup
//! - Service initialization with health checks
//! - Repository detection
//! - Result interpretation
//!
//! # Prerequisites
//!
//! 1. Start Ollama: `ollama serve`
//! 2. Pull a model: `ollama pull qwen2.5-coder:7b`
//! 3. Set environment variables (optional):
//!    - `AIPACK_OLLAMA_ENDPOINT`: Ollama endpoint (default: http://localhost:11434)
//!    - `AIPACK_OLLAMA_MODEL`: Model name (default: qwen2.5-coder:7b)
//!
//! # Usage
//!
//! ```bash
//! # Detect the current directory
//! cargo run --example ollama_detect
//!
//! # Detect a specific repository
//! cargo run --example ollama_detect /path/to/repo
//!
//! # Use a different model
//! AIPACK_OLLAMA_MODEL=llama2 cargo run --example ollama_detect
//! ```

use aipack::{AipackConfig, DetectionService, ServiceError};
use std::env;
use std::path::PathBuf;
use std::process;

#[tokio::main]
async fn main() {
    // Initialize logging
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    println!("🚀 aipack - Ollama Detection Example\n");

    // Get repository path from command line or use current directory
    let repo_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    println!("📁 Repository: {}", repo_path.display());

    // Set backend to Ollama explicitly
    env::set_var("AIPACK_BACKEND", "ollama");

    // Load configuration
    let config = AipackConfig::default();

    println!("\n⚙️  Configuration:");
    println!("   Provider: {:?}", config.provider);
    println!("   Model: {}", config.model);
    println!("   Request Timeout: {}s", config.request_timeout_secs);

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("\n❌ Configuration error: {}", e);
        process::exit(1);
    }

    println!("\n🔧 Initializing detection service...");

    // Create detection service
    let service = match DetectionService::new(&config).await {
        Ok(svc) => {
            println!("✅ Service initialized");
            println!("   Backend: {}", svc.backend_name());
            if let Some(info) = svc.backend_model_info() {
                println!("   Model Info: {}", info);
            }
            svc
        }
        Err(e) => {
            eprintln!("\n❌ Failed to initialize service: {}", e);
            eprintln!("\n💡 Troubleshooting:");
            eprintln!("   1. Ensure Ollama is running: ollama serve");
            eprintln!("   2. Pull the model: ollama pull {}", config.model);
            eprintln!("   3. Check OLLAMA_HOST environment variable if using custom endpoint");
            process::exit(1);
        }
    };

    println!("\n🔍 Analyzing repository...");

    // Perform detection
    let start = std::time::Instant::now();
    let result = match service.detect(repo_path.clone()).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("\n❌ Detection failed: {}", e);

            match e {
                ServiceError::PathNotFound(_) => {
                    eprintln!("\n💡 The specified path does not exist");
                }
                ServiceError::NotADirectory(_) => {
                    eprintln!("\n💡 The path must be a directory, not a file");
                }
                ServiceError::AnalysisError(ae) => {
                    eprintln!("\n💡 Repository analysis failed: {}", ae);
                }
                ServiceError::BackendError(be) => {
                    eprintln!("\n💡 LLM backend error: {}", be);
                    eprintln!("   This could be due to:");
                    eprintln!("   - Ollama service stopped");
                    eprintln!("   - Model not available");
                    eprintln!("   - Network timeout");
                }
                _ => {}
            }

            process::exit(1);
        }
    };

    let elapsed = start.elapsed();

    println!(
        "\n✅ Detection completed in {:.2}s\n",
        elapsed.as_secs_f64()
    );

    // Display results
    println!("{}", result);

    // Additional analysis
    println!("\n📊 Analysis:");
    println!("   Confidence Level: {}", result.confidence_level());

    if result.is_high_confidence() {
        println!("   ✅ High confidence detection - commands are likely correct");
    } else if result.is_low_confidence() {
        println!("   ⚠️  Low confidence - please review commands carefully");
    }

    if result.has_warnings() {
        println!("\n⚠️  Please review the warnings above before using these commands");
    }

    // Example of using the commands
    println!("\n💡 Usage Examples:");
    println!("   Build:  {}", result.build_command);
    println!("   Test:   {}", result.test_command);

    if let Some(ref dev_cmd) = result.dev_command {
        println!("   Dev:    {}", dev_cmd);
    }

    // Show detected files
    if !result.detected_files.is_empty() {
        println!("\n📄 Key files used for detection:");
        for file in &result.detected_files {
            println!("   - {}", file);
        }
    }

    println!("\n✨ Done!");
}

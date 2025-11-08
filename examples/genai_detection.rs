//! Example: Using GenAI backend for multi-provider LLM detection
//!
//! This example demonstrates how to use the GenAI backend to detect build systems
//! using different LLM providers (Ollama, Claude, OpenAI, etc.).
//!
//! For complete documentation on environment variables and provider setup,
//! see `CLAUDE.md` section "Environment Variables for GenAI Backend".
//!
//! # Quick Start
//!
//! ```bash
//! # Ollama (default, no API key needed)
//! cargo run --example genai_detection
//!
//! # Claude (requires ANTHROPIC_API_KEY)
//! PROVIDER=claude ANTHROPIC_API_KEY=sk-ant-... cargo run --example genai_detection
//!
//! # OpenAI (requires OPENAI_API_KEY)
//! PROVIDER=openai OPENAI_API_KEY=sk-proj-... cargo run --example genai_detection
//!
//! # Gemini (requires GOOGLE_API_KEY)
//! PROVIDER=gemini GOOGLE_API_KEY=AIza... cargo run --example genai_detection
//! ```

use aipack::ai::genai_backend::{GenAIBackend, Provider};
use aipack::detection::types::RepositoryContext;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,aipack=debug")
        .init();

    // Determine which provider to use from environment
    let provider_str = env::var("PROVIDER").unwrap_or_else(|_| "ollama".to_string());
    let provider = match provider_str.as_str() {
        "ollama" => Provider::Ollama,
        "claude" => Provider::Claude,
        "openai" => Provider::OpenAI,
        "gemini" => Provider::Gemini,
        "grok" => Provider::Grok,
        "groq" => Provider::Groq,
        _ => {
            eprintln!("Unknown provider: {}", provider_str);
            eprintln!("Supported providers: ollama, claude, openai, gemini, grok, groq");
            std::process::exit(1);
        }
    };

    // Get model name from environment or use default
    let model = match provider {
        Provider::Ollama => {
            env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".to_string())
        }
        Provider::Claude => env::var("CLAUDE_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-5-20250929".to_string()),
        Provider::OpenAI => env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4".to_string()),
        Provider::Gemini => {
            env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-pro".to_string())
        }
        Provider::Grok => env::var("GROK_MODEL").unwrap_or_else(|_| "grok-1".to_string()),
        Provider::Groq => {
            env::var("GROQ_MODEL").unwrap_or_else(|_| "mixtral-8x7b-32768".to_string())
        }
    };

    println!("🔧 Creating {} backend with model: {}", provider_str, model);

    // Create GenAI backend
    // Note: genai automatically reads API keys from environment variables:
    // - ANTHROPIC_API_KEY for Claude
    // - OPENAI_API_KEY for OpenAI
    // - GOOGLE_API_KEY for Gemini
    // - XAI_API_KEY for Grok
    // - GROQ_API_KEY for Groq
    // - OLLAMA_HOST for custom Ollama endpoint (optional)
    //
    // You do NOT need to pass API keys or endpoints as parameters!
    let backend = GenAIBackend::new(provider, model).await?;

    println!("✅ Backend created: {}", backend.name());
    if let Some(info) = backend.model_info() {
        println!("   Model: {}", info);
    }

    // Get repository path from args or use current directory
    let repo_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    println!("\n📂 Analyzing repository: {}", repo_path.display());

    // Create a simple repository context (you'd normally use the full analyzer)
    let file_tree = format!(
        "{}/\n├── Cargo.toml\n├── src/\n│   ├── main.rs\n│   └── lib.rs\n└── README.md",
        repo_path.file_name().unwrap().to_string_lossy()
    );

    let context = RepositoryContext::minimal(repo_path.clone(), file_tree);

    println!("\n🤖 Detecting build system using {}...\n", backend.name());

    // Perform detection
    match backend.detect(context).await {
        Ok(result) => {
            println!("✅ Detection successful!\n");
            println!("Build System: {}", result.build_system);
            println!("Language: {}", result.language);
            println!("Confidence: {:.1}%", result.confidence * 100.0);
            println!("\nBuild Command: {}", result.build_command);
            println!("Test Command: {}", result.test_command);

            if let Some(dev_cmd) = result.dev_command {
                println!("Dev Command: {}", dev_cmd);
            }

            if !result.detected_files.is_empty() {
                println!("\nDetected Files:");
                for file in &result.detected_files {
                    println!("  - {}", file);
                }
            }

            println!("\n⏱️  Processing Time: {}ms", result.processing_time_ms);
        }
        Err(e) => {
            eprintln!("❌ Detection failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

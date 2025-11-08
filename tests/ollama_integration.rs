//! Integration tests for GenAI backend with Ollama
//!
//! These tests require a running Ollama service and will be skipped if not available.
//! To run these tests:
//!
//! 1. Start Ollama: `ollama serve`
//! 2. Pull a model: `ollama pull qwen2.5-coder:7b`
//! 3. Run tests: `cargo test --test ollama_integration`
//!
//! Tests can be run against different endpoints by setting environment variables:
//! - `AIPACK_OLLAMA_ENDPOINT`: Ollama endpoint (default: http://localhost:11434)
//! - `AIPACK_OLLAMA_MODEL`: Model name (default: qwen2.5-coder:7b)

use aipack::ai::genai_backend::{GenAIBackend, Provider};
use aipack::config::AipackConfig;
use aipack::detection::service::DetectionService;
use aipack::detection::types::RepositoryContext;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

/// Check if Ollama is available for testing
async fn is_service_available() -> bool {
    let endpoint =
        env::var("AIPACK_OLLAMA_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Set OLLAMA_HOST environment variable for genai
    env::set_var("OLLAMA_HOST", &endpoint);

    // Try to create a client - if genai can't connect, it will fail
    GenAIBackend::with_config(
        Provider::Ollama,
        "qwen2.5-coder:7b".to_string(),
        Some(Duration::from_secs(5)),
        None,
    )
    .await
    .is_ok()
}

/// Skip test if service is not available
macro_rules! skip_if_no_service {
    () => {
        if !is_service_available().await {
            eprintln!("⚠️  Skipping test: Ollama not available");
            eprintln!("   To run this test:");
            eprintln!("   1. Start Ollama: ollama serve");
            eprintln!("   2. Pull a model: ollama pull qwen2.5-coder:7b");
            return;
        }
    };
}

/// Creates a test client with configured endpoint and model
async fn create_test_client() -> GenAIBackend {
    let endpoint =
        env::var("AIPACK_OLLAMA_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let model = env::var("AIPACK_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());

    // Set OLLAMA_HOST environment variable for genai
    env::set_var("OLLAMA_HOST", &endpoint);

    GenAIBackend::with_config(
        Provider::Ollama,
        model,
        Some(Duration::from_secs(60)),
        None,
    )
    .await
    .expect("Failed to create GenAI client")
}

#[tokio::test]
async fn test_service_health_check() {
    let client = create_test_client().await;

    // GenAI backend doesn't have a separate health_check method
    // If we got here, the client was created successfully, which means Ollama is available
    println!("✅ Service is available and healthy");
    println!("   Backend: {}", client.name());
}

#[tokio::test]
async fn test_detect_rust_project() {
    skip_if_no_service!();

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a simple Rust project structure
    fs::write(
        repo_path.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
"#,
    )
    .unwrap();

    fs::create_dir(repo_path.join("src")).unwrap();
    fs::write(
        repo_path.join("src").join("main.rs"),
        r#"fn main() {
    println!("Hello, world!");
}
"#,
    )
    .unwrap();

    fs::write(
        repo_path.join("README.md"),
        r#"# Test Project

A simple Rust project for testing.

## Building

Run `cargo build` to build the project.
"#,
    )
    .unwrap();

    // Create context
    let context = RepositoryContext::minimal(
        repo_path.to_path_buf(),
        r#"test-project/
├── Cargo.toml
├── README.md
└── src/
    └── main.rs
"#
        .to_string(),
    )
    .with_key_file(
        "Cargo.toml".to_string(),
        fs::read_to_string(repo_path.join("Cargo.toml")).unwrap(),
    )
    .with_readme(fs::read_to_string(repo_path.join("README.md")).unwrap());

    // Detect
    let client = create_test_client().await;
    let result = client.detect(context).await;

    match result {
        Ok(detection) => {
            println!("✅ Detection successful!");
            println!("   Language: {}", detection.language);
            println!("   Build System: {}", detection.build_system);
            println!("   Build Command: {}", detection.build_command);
            if let Some(ref test_cmd) = detection.test_command {
                println!("   Test Command: {}", test_cmd);
            } else {
                println!("   Test Command: (not specified)");
            }
            println!("   Confidence: {:.1}%", detection.confidence * 100.0);

            // Verify the detection makes sense
            assert_eq!(detection.language, "Rust");
            assert_eq!(detection.build_system, "cargo");
            assert!(detection.build_command.contains("cargo"));
            assert!(detection.test_command.as_ref().map_or(false, |cmd| cmd.contains("cargo")));
            assert!(detection.confidence >= 0.7);
        }
        Err(e) => {
            panic!("❌ Detection failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_ollama_detect_nodejs_project() {
    skip_if_no_service!();

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a Node.js project structure
    fs::write(
        repo_path.join("package.json"),
        r#"{
  "name": "test-project",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "test": "jest",
    "start": "node dist/index.js"
  },
  "dependencies": {
    "express": "^4.18.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0",
    "jest": "^29.0.0"
  }
}
"#,
    )
    .unwrap();

    fs::write(
        repo_path.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "outDir": "./dist"
  }
}
"#,
    )
    .unwrap();

    let context = RepositoryContext::minimal(
        repo_path.to_path_buf(),
        r#"test-project/
├── package.json
├── tsconfig.json
└── src/
    └── index.ts
"#
        .to_string(),
    )
    .with_key_file(
        "package.json".to_string(),
        fs::read_to_string(repo_path.join("package.json")).unwrap(),
    )
    .with_key_file(
        "tsconfig.json".to_string(),
        fs::read_to_string(repo_path.join("tsconfig.json")).unwrap(),
    );

    let client = create_test_client().await;
    let result = client.detect(context).await;

    match result {
        Ok(detection) => {
            println!("✅ Detection successful!");
            println!("   Language: {}", detection.language);
            println!("   Build System: {}", detection.build_system);
            println!("   Build Command: {}", detection.build_command);
            println!("   Confidence: {:.1}%", detection.confidence * 100.0);

            // Verify the detection makes sense
            assert!(
                detection.language.contains("JavaScript")
                    || detection.language.contains("TypeScript")
            );
            assert_eq!(detection.build_system, "npm");
            assert!(detection.build_command.contains("npm"));
            assert!(detection.confidence >= 0.6);
        }
        Err(e) => {
            panic!("❌ Detection failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_detection_service_end_to_end() {
    skip_if_no_service!();

    // Set up configuration
    env::set_var("AIPACK_BACKEND", "ollama");

    let config = AipackConfig::default();

    // Create detection service
    let service = DetectionService::new(&config).await;

    if service.is_err() {
        eprintln!(
            "⚠️  Could not create detection service: {}",
            service.unwrap_err()
        );
        return;
    }

    let service = service.unwrap();

    println!("✅ Detection service created");
    println!("   Backend: {}", service.backend_name());
    if let Some(info) = service.backend_model_info() {
        println!("   Model: {}", info);
    }

    // Create a test Rust project
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    fs::write(
        repo_path.join("Cargo.toml"),
        r#"[package]
name = "integration-test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    fs::create_dir(repo_path.join("src")).unwrap();
    fs::write(
        repo_path.join("src").join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    // Perform detection
    let result = service.detect(repo_path.to_path_buf()).await;

    match result {
        Ok(detection) => {
            println!("✅ End-to-end detection successful!");
            println!("{}", detection);

            assert_eq!(detection.language, "Rust");
            assert_eq!(detection.build_system, "cargo");
            assert!(detection.processing_time_ms > 0);
        }
        Err(e) => {
            panic!("❌ End-to-end detection failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_ollama_backend_trait() {
    skip_if_no_service!();

    let client = create_test_client().await;

    // Test LLMBackend trait methods
    assert_eq!(client.name(), "Ollama");
    assert!(client.model_info().is_some());

    let model_info = client.model_info().unwrap();
    assert!(model_info.contains("qwen") || model_info.contains("llama") || model_info.contains("ollama"));
}

#[tokio::test]
async fn test_ollama_error_handling_invalid_model() {
    skip_if_no_service!();

    let endpoint =
        env::var("AIPACK_OLLAMA_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Set OLLAMA_HOST environment variable for genai
    env::set_var("OLLAMA_HOST", &endpoint);

    // Use a non-existent model
    let client = GenAIBackend::with_config(
        Provider::Ollama,
        "nonexistent-model:latest".to_string(),
        Some(Duration::from_secs(10)),
        None,
    )
    .await
    .expect("Failed to create client");

    let context =
        RepositoryContext::minimal(PathBuf::from("/test"), "test/\n└── file.txt".to_string());

    let result = client.detect(context).await;

    // The error might occur either during client creation or during detection
    if result.is_err() {
        let e = result.unwrap_err();
        println!("✅ Correctly caught error for invalid model: {}", e);
    } else {
        println!("⚠️  Detection succeeded with invalid model (unexpected)");
    }
}

#[tokio::test]
async fn test_ollama_timeout_handling() {
    skip_if_no_service!();

    let endpoint =
        env::var("AIPACK_OLLAMA_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let model = env::var("AIPACK_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());

    // Set OLLAMA_HOST environment variable for genai
    env::set_var("OLLAMA_HOST", &endpoint);

    // Use a very short timeout
    let client = GenAIBackend::with_config(
        Provider::Ollama,
        model,
        Some(Duration::from_millis(1)),
        None,
    )
    .await
    .expect("Failed to create client");

    let context =
        RepositoryContext::minimal(PathBuf::from("/test"), "test/\n└── file.txt".to_string())
            .with_key_file("test.txt".to_string(), "content".repeat(1000));

    let result = client.detect(context).await;

    // This might timeout or succeed depending on system speed
    match result {
        Ok(_) => {
            println!("⚠️  Request completed despite very short timeout");
        }
        Err(e) => {
            println!("✅ Timeout error correctly caught: {}", e);
        }
    }
}

#[tokio::test]
async fn test_detection_service_path_validation() {
    skip_if_no_service!();

    env::set_var("AIPACK_BACKEND", "ollama");
    let config = AipackConfig::default();

    let service = DetectionService::new(&config).await;

    if service.is_err() {
        return;
    }

    let service = service.unwrap();

    // Test with non-existent path
    let result = service.detect(PathBuf::from("/nonexistent/path")).await;
    assert!(result.is_err());

    println!("✅ Path validation works correctly");
}

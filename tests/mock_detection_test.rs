//! Integration tests using MockLLMClient for detection logic
//!
//! These tests verify the detection flow without requiring a real LLM backend.

use aipack::fs::MockFileSystem;
use aipack::llm::{MockLLMClient, MockResponse};
use aipack::{FileSystem, LLMClient, Validator};
use serde_json::json;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Creates a mock file system with a Rust project structure
fn create_rust_project_fs() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // Create Cargo.toml
    std::fs::write(
        repo_path.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
"#,
    )
    .unwrap();

    // Create src directory and main.rs
    std::fs::create_dir(repo_path.join("src")).unwrap();
    std::fs::write(
        repo_path.join("src/main.rs"),
        r#"fn main() {
    println!("Hello, world!");
}
"#,
    )
    .unwrap();

    (temp_dir, repo_path)
}

/// Creates a valid UniversalBuild JSON for Rust
fn create_rust_universal_build() -> serde_json::Value {
    json!({
        "version": "1.0",
        "metadata": {
            "project_name": "test-project",
            "language": "rust",
            "build_system": "cargo",
            "confidence": 0.95,
            "reasoning": "Found Cargo.toml with standard Rust project structure"
        },
        "build": {
            "base": "rust:1.75",
            "packages": [],
            "env": {},
            "commands": ["cargo build --release"],
            "context": [{"from": ".", "to": "/app"}],
            "cache": ["/usr/local/cargo/registry", "target"],
            "artifacts": ["target/release/test-project"]
        },
        "runtime": {
            "base": "debian:bookworm-slim",
            "packages": ["ca-certificates"],
            "env": {},
            "copy": [{"from": "target/release/test-project", "to": "/app/test-project"}],
            "command": ["/app/test-project"],
            "ports": []
        }
    })
}

#[tokio::test]
async fn test_mock_client_basic_functionality() {
    let client = MockLLMClient::new();

    // Add a simple text response
    client.add_response(MockResponse::text("Hello, I'm a mock LLM!"));

    assert_eq!(client.remaining_responses(), 1);
    assert_eq!(client.name(), "MockLLM");
}

#[tokio::test]
async fn test_mock_client_with_tool_calls() {
    let client = MockLLMClient::new();

    // Simulate LLM requesting to read Cargo.toml
    let tool_call = MockLLMClient::read_file_call("call_1", "Cargo.toml");
    client.add_response(MockResponse::with_tool_calls(
        "Let me check the Cargo.toml file",
        vec![tool_call],
    ));

    assert_eq!(client.remaining_responses(), 1);
}

#[tokio::test]
async fn test_mock_client_sequence() {
    let client = MockLLMClient::new();

    // Set up a sequence of responses simulating detection workflow
    client.add_responses(vec![
        // First: LLM asks to list files
        MockResponse::with_tool_calls(
            "Let me explore the repository",
            vec![MockLLMClient::list_files_call("call_1", ".")],
        ),
        // Second: LLM asks to read Cargo.toml after seeing the file list
        MockResponse::with_tool_calls(
            "I see a Cargo.toml, let me read it",
            vec![MockLLMClient::read_file_call("call_2", "Cargo.toml")],
        ),
        // Third: LLM asks for best practices
        MockResponse::with_tool_calls(
            "This is a Rust project, let me get best practices",
            vec![MockLLMClient::get_best_practices_call(
                "call_3", "rust", "cargo",
            )],
        ),
        // Fourth: LLM submits detection
        MockResponse::with_tool_calls(
            "I have enough information to submit",
            vec![MockLLMClient::submit_detection_call(
                "call_4",
                create_rust_universal_build(),
            )],
        ),
    ]);

    assert_eq!(client.remaining_responses(), 4);
}

#[tokio::test]
async fn test_mock_file_system() {
    let fs = MockFileSystem::new();

    // Add files to mock filesystem
    fs.add_file(
        "Cargo.toml",
        r#"[package]
name = "test"
version = "0.1.0"
"#,
    );
    fs.add_file("src/main.rs", "fn main() {}");

    // Test file existence
    assert!(fs.exists(Path::new("Cargo.toml")));
    assert!(fs.exists(Path::new("src/main.rs")));
    assert!(!fs.exists(Path::new("nonexistent.txt")));

    // Test file reading
    let content = fs.read_to_string(Path::new("Cargo.toml")).unwrap();
    assert!(content.contains("name = \"test\""));
}

#[tokio::test]
async fn test_real_file_system_integration() {
    // Create a real temporary directory with files
    let (_temp_dir, repo_path) = create_rust_project_fs();

    // Verify files exist
    assert!(repo_path.join("Cargo.toml").exists());
    assert!(repo_path.join("src/main.rs").exists());

    // Read file content
    let content = std::fs::read_to_string(repo_path.join("Cargo.toml")).unwrap();
    assert!(content.contains("name = \"test-project\""));
}

#[tokio::test]
async fn test_universal_build_validation() {
    let build = create_rust_universal_build();

    // Verify the JSON structure is valid
    let parsed: aipack::output::schema::UniversalBuild =
        serde_json::from_value(build).expect("Should parse as UniversalBuild");

    // Validate the build
    assert!(Validator::new().validate(&parsed).is_ok());

    // Check key fields
    assert_eq!(parsed.metadata.language, "rust");
    assert_eq!(parsed.metadata.build_system, "cargo");
}

#[test]
fn test_tool_call_helpers() {
    // Test read_file helper
    let read_call = MockLLMClient::read_file_call("id1", "path/to/file.txt");
    assert_eq!(read_call.name, "read_file");
    assert_eq!(read_call.call_id, "id1");
    assert_eq!(read_call.arguments["path"], "path/to/file.txt");

    // Test list_files helper
    let list_call = MockLLMClient::list_files_call("id2", "src");
    assert_eq!(list_call.name, "list_files");
    assert_eq!(list_call.arguments["path"], "src");

    // Test get_best_practices helper
    let bp_call = MockLLMClient::get_best_practices_call("id3", "rust", "cargo");
    assert_eq!(bp_call.name, "get_best_practices");
    assert_eq!(bp_call.arguments["language"], "rust");
    assert_eq!(bp_call.arguments["build_system"], "cargo");

    // Test submit_detection helper
    let detection = json!({"version": "1.0"});
    let submit_call = MockLLMClient::submit_detection_call("id4", detection.clone());
    assert_eq!(submit_call.name, "submit_detection");
    assert_eq!(submit_call.arguments, detection);
}

#[tokio::test]
async fn test_mock_client_error_handling() {
    let client = MockLLMClient::new();

    // Add an error response
    client.add_response(MockResponse::error(aipack::BackendError::TimeoutError {
        seconds: 30,
    }));

    // Try to make a request
    use aipack::llm::{LLMClient, LLMRequest};
    let result = client.chat(LLMRequest::new(vec![])).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_client_exhausted() {
    let client = MockLLMClient::new();

    // Don't add any responses
    use aipack::llm::{LLMClient, LLMRequest};
    let result = client.chat(LLMRequest::new(vec![])).await;

    assert!(result.is_err());
}

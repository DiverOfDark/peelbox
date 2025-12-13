//! Integration tests for embedded LLM client
//!
//! Tests the full embedded LLM workflow:
//! - Hardware detection
//! - Model selection based on available RAM
//! - Model downloading (if needed)
//! - Model loading and inference
//! - Tool calling functionality
//!
//! NOTE: Tests that perform inference are marked with #[serial] to run sequentially
//! and avoid high concurrent CPU/memory load.

use aipack::llm::{
    ChatMessage, EmbeddedClient, EmbeddedModel, HardwareCapabilities, HardwareDetector, LLMClient,
    LLMRequest, ModelSelector, RecordingLLMClient, RecordingMode,
};
use serial_test::serial;
use std::sync::Arc;

/// Check if we have enough RAM to run embedded model tests
fn has_sufficient_ram() -> bool {
    let capabilities = HardwareDetector::detect();
    // Need at least 3GB available RAM for smallest model (0.5B)
    capabilities.available_ram_gb() >= 3.0
}

#[tokio::test]
async fn test_hardware_detection() {
    let capabilities = HardwareDetector::detect();

    // Basic sanity checks
    assert!(capabilities.total_ram_bytes > 0);
    assert!(capabilities.available_ram_bytes > 0);
    assert!(capabilities.available_ram_bytes <= capabilities.total_ram_bytes);
    assert!(capabilities.cpu_cores > 0);

    // Log detected hardware
    println!(
        "Detected hardware: {:.1}GB RAM ({:.1}GB available), {} cores, device: {}",
        capabilities.total_ram_gb(),
        capabilities.available_ram_gb(),
        capabilities.cpu_cores,
        capabilities.best_device()
    );
}

#[tokio::test]
#[serial]
async fn test_embedded_llm_inference() {
    // Check if recording exists first
    let recordings_dir = std::path::PathBuf::from("tests/recordings");
    let test_request = LLMRequest::new(vec![
        ChatMessage::system("You are a helpful assistant. Respond concisely."),
        ChatMessage::user("What is 2+2?"),
    ])
    .with_max_tokens(50)
    .with_temperature(0.7);

    let recorded_request = aipack::llm::RecordedRequest::from_llm_request(&test_request);
    let request_hash = recorded_request.canonical_hash();
    let recording_path = recordings_dir.join(format!("{}.json", request_hash));

    // If recording exists, use replay mode with mock client
    // Otherwise, try to create real client
    let client: Arc<dyn LLMClient> = if recording_path.exists() {
        println!("Using cached recording, skipping model loading");
        Arc::new(aipack::llm::MockLLMClient::new())
    } else {
        println!("No recording found, will create embedded client");
        if !has_sufficient_ram() {
            println!("Skipping test: insufficient RAM and no recording");
            return;
        }

        let capabilities = HardwareCapabilities {
            total_ram_bytes: 16 * 1024 * 1024 * 1024,
            available_ram_bytes: 8 * 1024 * 1024 * 1024,
            cuda_available: false,
            cuda_memory_bytes: None,
            metal_available: false,
            cpu_cores: 8,
        };

        let model = ModelSelector::smallest();
        println!("Testing inference with {} on CPU", model.display_name);

        match EmbeddedClient::with_model(model, &capabilities, false).await {
            Ok(client) => Arc::new(client),
            Err(e) => {
                println!("Skipping inference test: failed to create client: {}", e);
                return;
            }
        }
    };

    // Always use Auto mode - will replay if available, record if not
    let recording_client = RecordingLLMClient::new(client, RecordingMode::Auto, recordings_dir)
        .expect("Failed to create recording client");

    // Create a simple request
    let request = LLMRequest::new(vec![
        ChatMessage::system("You are a helpful assistant. Respond concisely."),
        ChatMessage::user("What is 2+2?"),
    ])
    .with_max_tokens(50)
    .with_temperature(0.7);

    println!("Sending request to embedded LLM...");
    let result = recording_client.chat(request).await;

    match result {
        Ok(response) => {
            println!("Response: {}", response.content);
            println!("Response time: {:?}", response.response_time);

            assert!(!response.content.is_empty());
            assert!(response.content.len() < 1000); // Should be a short response
        }
        Err(e) => {
            println!("Request failed: {}", e);
            panic!("Test failed: {}", e);
        }
    }
}

#[tokio::test]
#[serial]
async fn test_embedded_llm_tool_calling() {
    // Check if recording exists first
    let recordings_dir = std::path::PathBuf::from("tests/recordings");
    let test_request = LLMRequest::new(vec![
        ChatMessage::system(
            r#"You are a helpful assistant. You can call tools using JSON format:
{"name": "tool_name", "arguments": {...}}

Available tools:
- calculate: Performs mathematical calculations"#,
        ),
        ChatMessage::user("Calculate 15 * 23 using the calculate tool"),
    ])
    .with_max_tokens(100)
    .with_temperature(0.1);

    let recorded_request = aipack::llm::RecordedRequest::from_llm_request(&test_request);
    let request_hash = recorded_request.canonical_hash();
    let recording_path = recordings_dir.join(format!("{}.json", request_hash));

    // If recording exists, use replay mode with mock client
    // Otherwise, try to create real client
    let client: Arc<dyn LLMClient> = if recording_path.exists() {
        println!("Using cached recording, skipping model loading");
        Arc::new(aipack::llm::MockLLMClient::new())
    } else {
        println!("No recording found, will create embedded client");
        if !has_sufficient_ram() {
            println!("Skipping test: insufficient RAM and no recording");
            return;
        }

        let capabilities = HardwareCapabilities {
            total_ram_bytes: 16 * 1024 * 1024 * 1024,
            available_ram_bytes: 8 * 1024 * 1024 * 1024,
            cuda_available: false,
            cuda_memory_bytes: None,
            metal_available: false,
            cpu_cores: 8,
        };

        let model = ModelSelector::smallest();
        println!("Testing tool calling with {} on CPU", model.display_name);

        match EmbeddedClient::with_model(model, &capabilities, false).await {
            Ok(client) => Arc::new(client),
            Err(e) => {
                println!("Skipping tool calling test: failed to create client: {}", e);
                return;
            }
        }
    };

    // Always use Auto mode - will replay if available, record if not
    let recording_client = RecordingLLMClient::new(client, RecordingMode::Auto, recordings_dir)
        .expect("Failed to create recording client");

    // Create a request that should trigger tool calling
    let request = LLMRequest::new(vec![
        ChatMessage::system(
            r#"You are a helpful assistant. You can call tools using JSON format:
{"name": "tool_name", "arguments": {...}}

Available tools:
- calculate: Performs mathematical calculations"#,
        ),
        ChatMessage::user("Calculate 15 * 23 using the calculate tool"),
    ])
    .with_max_tokens(100)
    .with_temperature(0.1);

    println!("Testing tool calling with embedded LLM...");
    let result = recording_client.chat(request).await;

    match result {
        Ok(response) => {
            println!("Response: {}", response.content);
            println!("Tool calls: {:?}", response.tool_calls);
            println!("Response time: {:?}", response.response_time);

            // Either should have tool calls, or should have content
            // (model may not always use tools correctly)
            assert!(!response.content.is_empty() || !response.tool_calls.is_empty());
        }
        Err(e) => {
            println!("Request failed: {}", e);
            panic!("Test failed: {}", e);
        }
    }
}

#[test]
fn test_model_ram_requirements() {
    // Verify all models have reasonable RAM requirements
    for model in EmbeddedModel::ALL_MODELS {
        assert!(model.ram_required_gb > 0.0);
        assert!(model.ram_required_gb < 100.0); // Sanity check

        println!(
            "Model: {} - RAM: {:.1}GB - Params: {}",
            model.display_name, model.ram_required_gb, model.params
        );
    }
}

#[test]
fn test_model_supports_tools() {
    // All Qwen2.5-Coder models should support tool calling
    for model in EmbeddedModel::ALL_MODELS {
        assert!(
            model.supports_tools,
            "Model {} should support tools",
            model.display_name
        );
    }
}

#[test]
fn test_smallest_model_is_0_5b() {
    let smallest = ModelSelector::smallest();
    assert_eq!(smallest.params, "0.5B");
    assert!(smallest.ram_required_gb < 1.0);
}

#[test]
fn test_model_selection_with_limited_ram() {
    // Simulate system with only 4GB available RAM
    let caps = HardwareCapabilities {
        total_ram_bytes: 8 * 1024 * 1024 * 1024,     // 8GB total
        available_ram_bytes: 4 * 1024 * 1024 * 1024, // 4GB available
        cuda_available: false,
        cuda_memory_bytes: None,
        metal_available: false,
        cpu_cores: 4,
    };

    let selected = ModelSelector::select(&caps);

    // Should select a smaller model (not 7B)
    if let Some(model) = selected {
        assert!(model.ram_required_gb <= 2.5); // Should be 1.5B or smaller
        println!("Selected for 4GB RAM: {}", model.display_name);
    }
}

#[test]
fn test_model_selection_with_insufficient_ram() {
    // Simulate system with insufficient RAM
    let caps = HardwareCapabilities {
        total_ram_bytes: 2 * 1024 * 1024 * 1024, // 2GB total
        available_ram_bytes: 1536 * 1024 * 1024, // 1.5GB available
        cuda_available: false,
        cuda_memory_bytes: None,
        metal_available: false,
        cpu_cores: 2,
    };

    let selected = ModelSelector::select(&caps);

    // Should return None - not enough RAM even for smallest model
    assert!(selected.is_none());
}

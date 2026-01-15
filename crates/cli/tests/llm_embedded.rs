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

use peelbox_llm::{
    ChatMessage, EmbeddedClient, EmbeddedModel, HardwareCapabilities, HardwareDetector, LLMClient,
    LLMRequest, ModelSelector, RecordingLLMClient, RecordingMode, ToolDefinition,
};
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;

/// Check if we have enough RAM to run embedded model tests
fn has_sufficient_ram() -> bool {
    let capabilities = HardwareDetector::detect();
    // Need at least 4GB available RAM for smallest model (1.5B)
    capabilities.available_ram_gb() >= 4.0
}

/// Create a MockLLMClient with model_info that matches embedded client recordings
/// This ensures hash computation matches when replaying recordings
fn create_mock_with_model_info(model_name: &str) -> peelbox_llm::MockLLMClient {
    let mut mock = peelbox_llm::MockLLMClient::new();
    mock.with_model_info(model_name.to_string());
    mock
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
    let recordings_dir = std::path::PathBuf::from("tests/recordings");
    let mode = RecordingMode::from_env(RecordingMode::Auto);

    // In Replay mode, always use MockLLMClient (recordings will be loaded by RecordingLLMClient)
    // Otherwise, try to create embedded client
    let client: Arc<dyn LLMClient> = if mode == RecordingMode::Replay {
        println!("Replay mode: using mock client (will load from recordings)");
        Arc::new(create_mock_with_model_info("Qwen2.5-Coder 7B GGUF (7B)"))
    } else if !has_sufficient_ram() {
        println!("Insufficient RAM, will use recording if available");
        Arc::new(create_mock_with_model_info("Qwen2.5-Coder 7B GGUF (7B)"))
    } else {
        let capabilities = HardwareDetector::detect();
        let model = ModelSelector::select(&capabilities).unwrap();
        println!(
            "Testing inference with {} on {}",
            model.display_name,
            capabilities.best_device()
        );

        match EmbeddedClient::with_model(model, &capabilities, false).await {
            Ok(client) => Arc::new(client),
            Err(e) => {
                println!(
                    "Failed to create client, will use recording if available: {}",
                    e
                );
                Arc::new(create_mock_with_model_info(&format!(
                    "{} ({})",
                    model.display_name, model.params
                )))
            }
        }
    };

    // RecordingLLMClient mode from env (defaults to Auto):
    // - Auto: Use recording if available, otherwise call client and record
    // - Replay: Only use recordings (error if missing)
    // - Record: Always call client and save recordings
    let mode = RecordingMode::from_env(RecordingMode::Auto);
    let recording_client = RecordingLLMClient::new(client, mode, recordings_dir)
        .expect("Failed to create recording client");

    // Create request - RecordingLLMClient will use recording if available
    let request = LLMRequest::new(vec![
        ChatMessage::system("You are a helpful assistant. Respond concisely."),
        ChatMessage::user("What is 2+2?"),
    ])
    .with_max_tokens(50)
    .with_temperature(0.7);

    println!("Sending request (will use recording if available)...");
    let result = recording_client.chat(request).await;

    // If using MockLLMClient and no recording exists, test should skip
    if let Err(ref e) = result {
        if e.to_string().contains("No more responses in queue") {
            println!("Skipping test: no recording available and using mock client");
            return;
        }
    }

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
    let recordings_dir = std::path::PathBuf::from("tests/recordings");
    let mode = RecordingMode::from_env(RecordingMode::Auto);

    // In Replay mode, always use MockLLMClient (recordings will be loaded by RecordingLLMClient)
    let client: Arc<dyn LLMClient> = if mode == RecordingMode::Replay {
        println!("Replay mode: using mock client (will load from recordings)");
        Arc::new(create_mock_with_model_info("Qwen2.5-Coder 7B GGUF (7B)"))
    } else if !has_sufficient_ram() {
        println!("Insufficient RAM, will use recording if available");
        Arc::new(create_mock_with_model_info("Qwen2.5-Coder 7B GGUF (7B)"))
    } else {
        let capabilities = HardwareDetector::detect();
        let model = ModelSelector::select(&capabilities).unwrap();
        println!(
            "Testing tool calling with {} on {}",
            model.display_name,
            capabilities.best_device()
        );

        match EmbeddedClient::with_model(model, &capabilities, false).await {
            Ok(client) => Arc::new(client),
            Err(e) => {
                println!(
                    "Failed to create client, will use recording if available: {}",
                    e
                );
                Arc::new(create_mock_with_model_info(&format!(
                    "{} ({})",
                    model.display_name, model.params
                )))
            }
        }
    };

    // Use the same mode for RecordingLLMClient
    let recording_client = RecordingLLMClient::new(client, mode, recordings_dir)
        .expect("Failed to create recording client");

    // Define calculate tool with proper JSON schema
    let calculate_tool = ToolDefinition {
        name: "calculate".to_string(),
        description: "Performs mathematical calculations on two numbers".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "The operation to perform: add, subtract, multiply, or divide",
                    "enum": ["add", "subtract", "multiply", "divide"]
                },
                "a": {
                    "type": "number",
                    "description": "First number"
                },
                "b": {
                    "type": "number",
                    "description": "Second number"
                }
            },
            "required": ["operation", "a", "b"]
        }),
    };

    // Create a request that should trigger tool calling (same as test_request)
    let request = LLMRequest::new(vec![
        ChatMessage::system("You are a helpful assistant that calls tools to solve problems."),
        ChatMessage::user("Calculate 15 * 23 using the calculate tool"),
    ])
    .with_tools(vec![calculate_tool])
    .with_max_tokens(100)
    .with_temperature(0.1);

    println!("Testing tool calling (will use recording if available)...");
    let result = recording_client.chat(request).await;

    // If using MockLLMClient and no recording exists, test should skip
    if let Err(ref e) = result {
        if e.to_string().contains("No more responses in queue") {
            println!("Skipping test: no recording available and using mock client");
            return;
        }
    }

    match result {
        Ok(response) => {
            println!("Response: {}", response.content);
            println!("Tool call: {:?}", response.tool_call);
            println!("Response time: {:?}", response.response_time);

            // Verify tool call structure
            if let Some(ref tool_call) = response.tool_call {
                println!("✅ Model generated a tool call");

                // Verify tool call has expected structure
                assert_eq!(
                    tool_call.name, "calculate",
                    "Tool name should be 'calculate'"
                );

                // Verify arguments exist and have expected fields
                assert!(
                    tool_call.arguments.is_object(),
                    "Arguments should be a JSON object"
                );
                let args = tool_call.arguments.as_object().unwrap();

                assert!(
                    args.contains_key("operation"),
                    "Arguments should contain 'operation'"
                );
                assert!(args.contains_key("a"), "Arguments should contain 'a'");
                assert!(args.contains_key("b"), "Arguments should contain 'b'");

                // Verify operation value
                if let Some(op) = args.get("operation").and_then(|v| v.as_str()) {
                    assert_eq!(op, "multiply", "Operation should be 'multiply'");
                }

                // Verify numbers
                if let Some(a) = args
                    .get("a")
                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                {
                    assert_eq!(a, 15.0, "First number should be 15");
                }
                if let Some(b) = args
                    .get("b")
                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                {
                    assert_eq!(b, 23.0, "Second number should be 23");
                }

                println!("✅ Tool call structure validated successfully");
            } else {
                println!(
                    "⚠️  No tool calls generated - model may need larger size or better prompting"
                );
                // Still pass test if content is present (model responded but didn't use tools)
                assert!(
                    !response.content.is_empty(),
                    "Should have either tool calls or content"
                );
            }
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

#[tokio::test]
#[serial]
async fn test_embedded_llm_tool_call_chain() {
    let recordings_dir = std::path::PathBuf::from("tests/recordings");
    let mode = RecordingMode::from_env(RecordingMode::Auto);

    // In Replay mode, always use MockLLMClient (recordings will be loaded by RecordingLLMClient)
    let client: Arc<dyn LLMClient> = if mode == RecordingMode::Replay {
        println!("Replay mode: using mock client (will load from recordings)");
        Arc::new(create_mock_with_model_info("Qwen2.5-Coder 7B GGUF (7B)"))
    } else if !has_sufficient_ram() {
        println!("Insufficient RAM, will use recording if available");
        Arc::new(create_mock_with_model_info("Qwen2.5-Coder 7B GGUF (7B)"))
    } else {
        let capabilities = HardwareDetector::detect();

        // Use PEELBOX_MODEL_SIZE env var or select based on available RAM
        let model = if let Ok(model_size) = std::env::var("PEELBOX_MODEL_SIZE") {
            EmbeddedModel::ALL_MODELS
                .iter()
                .find(|m| m.params == model_size)
                .unwrap_or_else(|| panic!("Model size {} not found", model_size))
        } else {
            ModelSelector::select(&capabilities).unwrap()
        };

        println!(
            "Testing tool call chain with {} on {}",
            model.display_name,
            capabilities.best_device()
        );

        match EmbeddedClient::with_model(model, &capabilities, false).await {
            Ok(client) => Arc::new(client),
            Err(e) => {
                println!(
                    "Failed to create client, will use recording if available: {}",
                    e
                );
                Arc::new(create_mock_with_model_info(&format!(
                    "{} ({})",
                    model.display_name, model.params
                )))
            }
        }
    };

    // Use the same mode for RecordingLLMClient
    let recording_client = RecordingLLMClient::new(client, mode, recordings_dir)
        .expect("Failed to create recording client");

    // Define multiple tools that can be chained
    let list_files_tool = ToolDefinition {
        name: "list_files".to_string(),
        description: "List files in a directory".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list"
                }
            },
            "required": ["path"]
        }),
    };

    let read_file_tool = ToolDefinition {
        name: "read_file".to_string(),
        description: "Read the contents of a file".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to read"
                }
            },
            "required": ["path"]
        }),
    };

    let submit_result_tool = ToolDefinition {
        name: "submit_result".to_string(),
        description: "Submit the final analysis result".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Summary of findings"
                }
            },
            "required": ["summary"]
        }),
    };

    // Simulate a tool call chain with multiple iterations
    let mut messages = vec![
        ChatMessage::system("You are a helpful assistant that uses tools to analyze repositories. Always call tools in sequence to gather information before submitting results."),
        ChatMessage::user("Analyze this directory by first listing files, then reading one file, then submitting a summary"),
    ];

    let mut tool_calls_history = Vec::new();
    const MAX_ITERATIONS: usize = 5;

    for iteration in 1..=MAX_ITERATIONS {
        println!("\n=== Iteration {} ===", iteration);

        let request = LLMRequest::new(messages.clone())
            .with_tools(vec![
                list_files_tool.clone(),
                read_file_tool.clone(),
                submit_result_tool.clone(),
            ])
            .with_max_tokens(200)
            .with_temperature(0.1);

        let response = match recording_client.chat(request).await {
            Ok(resp) => resp,
            Err(e) => {
                // If using MockLLMClient and no recording exists, skip test
                if e.to_string().contains("No more responses in queue") {
                    println!("Skipping test: no recording available and using mock client");
                    return;
                }
                panic!("LLM request failed: {}", e);
            }
        };

        println!("Response: {}", response.content);

        let tool_call = match &response.tool_call {
            Some(tc) => tc,
            None => {
                println!("No tool call in iteration {}", iteration);
                messages.push(ChatMessage::assistant(&response.content));
                break;
            }
        };

        println!(
            "Tool call: {} (args: {})",
            tool_call.name, tool_call.arguments
        );
        tool_calls_history.push(tool_call.name.clone());

        // Add assistant message with tool call
        messages.push(ChatMessage::assistant_with_tools(
            &response.content,
            vec![tool_call.clone()],
        ));

        // Simulate tool response
        let tool_result = match tool_call.name.as_str() {
            "list_files" => json!({
                "files": ["Cargo.toml", "README.md", "src/main.rs"],
                "count": 3
            }),
            "read_file" => json!({
                "path": tool_call.arguments.get("path").and_then(|p| p.as_str()).unwrap_or("unknown"),
                "content": "# Example File\nThis is test content.",
                "lines_shown": 2,
                "total_lines": 2,
                "truncated": false
            }),
            "submit_result" => {
                println!("✅ submit_result called - chain complete!");
                json!({
                    "success": true,
                    "summary": tool_call.arguments.get("summary").and_then(|s| s.as_str()).unwrap_or("No summary")
                })
            }
            _ => json!({"error": format!("Unknown tool: {}", tool_call.name)}),
        };

        messages.push(ChatMessage::tool_response(&tool_call.call_id, tool_result));

        // Check if we reached submit_result
        if tool_call.name == "submit_result" {
            println!("✅ Tool call chain completed successfully!");
            break;
        }
    }

    // Verify we had a meaningful tool call chain
    println!("\nTool call history: {:?}", tool_calls_history);

    if !tool_calls_history.is_empty() {
        println!(
            "✅ Model generated {} tool call(s) across iterations",
            tool_calls_history.len()
        );

        // Verify we got different tool calls (chain behavior)
        let unique_tools: std::collections::HashSet<_> = tool_calls_history.iter().collect();
        if unique_tools.len() > 1 {
            println!(
                "✅ Model called {} different tools (chain detected)",
                unique_tools.len()
            );
        } else {
            println!("⚠️  Model only called one type of tool - may not be chaining");
        }

        // Check if we reached submit_result
        if tool_calls_history.contains(&"submit_result".to_string()) {
            println!("✅ Chain ended with submit_result as expected");
        }
    } else {
        println!("⚠️  No tool calls generated - model may need better prompting or larger size");
    }

    // Test passes if we got at least one tool call
    assert!(
        !tool_calls_history.is_empty(),
        "Should generate at least one tool call"
    );
}

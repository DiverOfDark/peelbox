use crate::detection::types::DetectionResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid confidence value: {0} (must be between 0.0 and 1.0)")]
    InvalidConfidence(f32),
    #[error("Invalid command for {0}: command cannot be empty")]
    InvalidCommand(String),
    #[error("Parse error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmResponse {
    language: Option<String>,
    build_system: Option<String>,
    build_command: Option<String>,
    test_command: Option<String>,
    runtime: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    entry_point: Option<String>,
    #[serde(default)]
    dev_command: Option<String>,
    confidence: Option<f32>,
    reasoning: Option<String>,
    #[serde(default)]
    warnings: Vec<String>,
}

pub fn parse_ollama_response(response: &str) -> Result<DetectionResult, ParseError> {
    let start = Instant::now();

    debug!("Parsing response ({} chars)", response.len());

    let json_str = extract_json_from_response(response)?;

    let llm_response: LlmResponse = serde_json::from_str(&json_str).map_err(|e| {
        warn!("JSON parse error: {}", e);
        ParseError::InvalidJson(format!(
            "{}: {}",
            e,
            json_str.chars().take(100).collect::<String>()
        ))
    })?;

    let mut result = convert_to_detection_result(llm_response)?;
    validate_detection_result(&result)?;
    result.processing_time_ms = start.elapsed().as_millis() as u64;

    debug!("Successfully parsed response in {:?}", start.elapsed());

    Ok(result)
}

pub fn extract_json_from_response(response: &str) -> Result<String, ParseError> {
    let trimmed = response.trim();

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Ok(trimmed.to_string());
    }

    if trimmed.contains("```") {
        return extract_from_markdown_block(trimmed);
    }

    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if start < end {
                return Ok(trimmed[start..=end].to_string());
            }
        }
    }

    Err(ParseError::InvalidJson(
        "No JSON object found in response".to_string(),
    ))
}

fn extract_from_markdown_block(text: &str) -> Result<String, ParseError> {
    let re = Regex::new(r"```(?:json)?\s*\n?([\s\S]*?)\n?```").unwrap();

    if let Some(captures) = re.captures(text) {
        if let Some(json_match) = captures.get(1) {
            let json = json_match.as_str().trim();
            if json.starts_with('{') && json.ends_with('}') {
                return Ok(json.to_string());
            }
        }
    }

    Err(ParseError::InvalidJson(
        "Could not extract JSON from markdown block".to_string(),
    ))
}

fn convert_to_detection_result(llm: LlmResponse) -> Result<DetectionResult, ParseError> {
    let language = llm
        .language
        .ok_or_else(|| ParseError::MissingField("language".to_string()))?;
    let build_system = llm
        .build_system
        .ok_or_else(|| ParseError::MissingField("build_system".to_string()))?;
    let build_command = llm
        .build_command
        .ok_or_else(|| ParseError::MissingField("build_command".to_string()))?;
    let runtime = llm
        .runtime
        .ok_or_else(|| ParseError::MissingField("runtime".to_string()))?;
    let entry_point = llm
        .entry_point
        .ok_or_else(|| ParseError::MissingField("entry_point".to_string()))?;
    let reasoning = llm
        .reasoning
        .ok_or_else(|| ParseError::MissingField("reasoning".to_string()))?;

    let confidence_raw = llm
        .confidence
        .ok_or_else(|| ParseError::MissingField("confidence".to_string()))?;
    let confidence = confidence_raw.clamp(0.0, 1.0);

    if confidence != confidence_raw {
        warn!(
            "Confidence value {} was out of range, clamped to {}",
            confidence_raw, confidence
        );
    }

    Ok(DetectionResult {
        build_system,
        language,
        build_command,
        test_command: llm.test_command,
        runtime,
        dependencies: llm.dependencies,
        entry_point,
        dev_command: llm.dev_command,
        confidence,
        reasoning,
        warnings: llm.warnings,
        detected_files: Vec::new(),
        processing_time_ms: 0,
    })
}

pub fn validate_detection_result(result: &DetectionResult) -> Result<(), ParseError> {
    if result.language.trim().is_empty() {
        return Err(ParseError::MissingField("language".to_string()));
    }

    if result.build_system.trim().is_empty() {
        return Err(ParseError::MissingField("build_system".to_string()));
    }

    if result.build_command.trim().is_empty() {
        return Err(ParseError::InvalidCommand("build_command".to_string()));
    }

    if let Some(ref test_cmd) = result.test_command {
        if test_cmd.trim().is_empty() {
            return Err(ParseError::InvalidCommand("test_command".to_string()));
        }
    }

    if result.runtime.trim().is_empty() {
        return Err(ParseError::MissingField("runtime".to_string()));
    }

    if result.entry_point.trim().is_empty() {
        return Err(ParseError::MissingField("entry_point".to_string()));
    }

    if !(0.0..=1.0).contains(&result.confidence) {
        return Err(ParseError::InvalidConfidence(result.confidence));
    }

    Ok(())
}

impl fmt::Display for LlmResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LlmResponse {{ language: {}, build_system: {}, confidence: {:.2} }}",
            self.language.as_ref().map_or("None", |s| s.as_str()),
            self.build_system.as_ref().map_or("None", |s| s.as_str()),
            self.confidence.unwrap_or(0.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ollama_response_valid() {
        let response = r#"{
            "language": "Rust",
            "build_system": "cargo",
            "build_command": "cargo build --release",
            "test_command": "cargo test",
            "runtime": "rust:1.75",
            "dependencies": [],
            "entry_point": "/app",
            "confidence": 0.95,
            "reasoning": "Standard Rust project",
            "warnings": []
        }"#;

        let result = parse_ollama_response(response).unwrap();
        assert_eq!(result.language, "Rust");
        assert_eq!(result.build_system, "cargo");
        assert_eq!(result.build_command, "cargo build --release");
        assert_eq!(result.test_command, Some("cargo test".to_string()));
        assert_eq!(result.runtime, "rust:1.75");
        assert_eq!(result.entry_point, "/app");
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.reasoning, "Standard Rust project");
        assert_eq!(result.warnings.len(), 0);
    }

    #[test]
    fn test_parse_ollama_response_with_dev_command() {
        let response = r#"{
            "language": "JavaScript",
            "build_system": "npm",
            "build_command": "npm run build",
            "test_command": "npm test",
            "runtime": "rust:1.75",
            "dependencies": [],
            "entry_point": "/app",
            "dev_command": "npm run dev",
            "confidence": 0.9,
            "reasoning": "Node.js project with package.json",
            "warnings": ["No lockfile found"]
        }"#;

        let result = parse_ollama_response(response).unwrap();
        assert_eq!(result.language, "JavaScript");
        assert_eq!(result.dev_command, Some("npm run dev".to_string()));
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0], "No lockfile found");
    }

    #[test]
    fn test_parse_ollama_response_with_null_dev_command() {
        let response = r#"{
            "language": "Python",
            "build_system": "pip",
            "build_command": "pip install -e .",
            "test_command": "pytest",
            "runtime": "rust:1.75",
            "dependencies": [],
            "entry_point": "/app",
            "dev_command": null,
            "confidence": 0.85,
            "reasoning": "Python project with setup.py",
            "warnings": []
        }"#;

        let result = parse_ollama_response(response).unwrap();
        assert_eq!(result.dev_command, None);
    }

    #[test]
    fn test_parse_ollama_response_with_null_test_command() {
        let response = r#"{
            "language": "JavaScript",
            "build_system": "npm",
            "build_command": "npm run build",
            "test_command": null,
            "runtime": "node:22-alpine3.17",
            "dependencies": ["nodejs"],
            "entry_point": "npm start",
            "confidence": 0.9,
            "reasoning": "JavaScript project without test suite",
            "warnings": ["No test command available"]
        }"#;

        let result = parse_ollama_response(response).unwrap();
        assert_eq!(result.test_command, None);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0], "No test command available");
    }

    #[test]
    fn test_extract_json_from_response_plain() {
        let response = r#"{"key": "value"}"#;
        let json = extract_json_from_response(response).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_response_with_whitespace() {
        let response = r#"

            {"key": "value"}

        "#;
        let json = extract_json_from_response(response).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_response_markdown_json() {
        let response = r#"```json
{
  "key": "value"
}
```"#;
        let json = extract_json_from_response(response).unwrap();
        // Just check that it contains the key-value pair, whitespace may vary
        assert!(json.contains("\"key\""));
        assert!(json.contains("\"value\""));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn test_extract_json_from_response_markdown_plain() {
        let response = r#"```
{"key": "value"}
```"#;
        let json = extract_json_from_response(response).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_response_embedded() {
        let response = r#"Here is the result: {"key": "value"} as requested."#;
        let json = extract_json_from_response(response).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_response_complex_embedded() {
        let response = r#"Based on the analysis, here's what I found:

{
  "language": "Rust",
  "build_system": "cargo"
}

Let me know if you need more details."#;

        let json = extract_json_from_response(response).unwrap();
        assert!(json.contains("\"language\": \"Rust\""));
        assert!(json.contains("\"build_system\": \"cargo\""));
    }

    #[test]
    fn test_extract_json_from_response_no_json() {
        let response = "This is just plain text with no JSON";
        let result = extract_json_from_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_detection_result_valid() {
        let result = DetectionResult {
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            build_command: "cargo build".to_string(),
            test_command: Some("cargo test".to_string()),
            runtime: "rust:1.75".to_string(),
            dependencies: vec![],
            entry_point: "/app".to_string(),
            dev_command: None,
            confidence: 0.9,
            reasoning: "Test".to_string(),
            warnings: vec![],
            detected_files: vec![],
            processing_time_ms: 0,
        };

        assert!(validate_detection_result(&result).is_ok());
    }

    #[test]
    fn test_validate_detection_result_empty_language() {
        let result = DetectionResult {
            language: "".to_string(),
            build_system: "cargo".to_string(),
            build_command: "cargo build".to_string(),
            test_command: Some("cargo test".to_string()),
            runtime: "rust:1.75".to_string(),
            dependencies: vec![],
            entry_point: "/app".to_string(),
            dev_command: None,
            confidence: 0.9,
            reasoning: "Test".to_string(),
            warnings: vec![],
            detected_files: vec![],
            processing_time_ms: 0,
        };

        let err = validate_detection_result(&result).unwrap_err();
        assert!(matches!(err, ParseError::MissingField(_)));
    }

    #[test]
    fn test_validate_detection_result_empty_build_command() {
        let result = DetectionResult {
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            build_command: "".to_string(),
            test_command: Some("cargo test".to_string()),
            runtime: "rust:1.75".to_string(),
            dependencies: vec![],
            entry_point: "/app".to_string(),
            dev_command: None,
            confidence: 0.9,
            reasoning: "Test".to_string(),
            warnings: vec![],
            detected_files: vec![],
            processing_time_ms: 0,
        };

        let err = validate_detection_result(&result).unwrap_err();
        assert!(matches!(err, ParseError::InvalidCommand(_)));
    }

    #[test]
    fn test_validate_detection_result_invalid_confidence() {
        let result = DetectionResult {
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            build_command: "cargo build".to_string(),
            test_command: Some("cargo test".to_string()),
            runtime: "rust:1.75".to_string(),
            dependencies: vec![],
            entry_point: "/app".to_string(),
            dev_command: None,
            confidence: 1.5,
            reasoning: "Test".to_string(),
            warnings: vec![],
            detected_files: vec![],
            processing_time_ms: 0,
        };

        let err = validate_detection_result(&result).unwrap_err();
        assert!(matches!(err, ParseError::InvalidConfidence(_)));
    }

    #[test]
    fn test_parse_ollama_response_clamps_confidence() {
        let response = r#"{
            "language": "Rust",
            "build_system": "cargo",
            "build_command": "cargo build",
            "test_command": "cargo test",
            "runtime": "rust:1.75",
            "dependencies": [],
            "entry_point": "/app",
            "confidence": 1.5,
            "reasoning": "Test",
            "warnings": []
        }"#;

        let result = parse_ollama_response(response).unwrap();
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_parse_ollama_response_missing_field() {
        let response = r#"{
            "language": "Rust",
            "build_command": "cargo build",
            "test_command": "cargo test",
            "runtime": "rust:1.75",
            "dependencies": [],
            "entry_point": "/app",
            "confidence": 0.9,
            "reasoning": "Test",
            "warnings": []
        }"#;

        let result = parse_ollama_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_llm_response_display() {
        let response = LlmResponse {
            language: Some("Rust".to_string()),
            build_system: Some("cargo".to_string()),
            build_command: Some("cargo build".to_string()),
            test_command: Some("cargo test".to_string()),
            runtime: Some("rust:1.75".to_string()),
            dependencies: vec![],
            entry_point: Some("/app".to_string()),
            dev_command: None,
            confidence: Some(0.95),
            reasoning: Some("Test".to_string()),
            warnings: vec![],
        };

        let display = format!("{}", response);
        assert!(display.contains("Rust"));
        assert!(display.contains("cargo"));
        assert!(display.contains("0.95"));
    }

    #[test]
    fn test_extract_from_markdown_multiple_blocks() {
        let response = r#"Here's some text
```json
{
  "key": "value"
}
```

More text"#;

        let json = extract_json_from_response(response).unwrap();
        assert!(json.contains("\"key\""));
        assert!(json.contains("\"value\""));
    }

    #[test]
    fn test_parse_error_display() {
        let error = ParseError::InvalidJson("test error".to_string());
        assert_eq!(error.to_string(), "Invalid JSON: test error");

        let error = ParseError::MissingField("language".to_string());
        assert_eq!(error.to_string(), "Missing required field: language");

        let error = ParseError::InvalidConfidence(1.5);
        assert_eq!(
            error.to_string(),
            "Invalid confidence value: 1.5 (must be between 0.0 and 1.0)"
        );

        let error = ParseError::InvalidCommand("build_command".to_string());
        assert_eq!(
            error.to_string(),
            "Invalid command for build_command: command cannot be empty"
        );
    }
}

//! End-to-end tests using fixtures and LLM recordings
//!
//! These tests verify the complete detection pipeline:
//! - Bootstrap scanning
//! - LLM conversation with tool calling
//! - Validation of final output
//!
//! Tests use RecordingLLMClient to replay cached LLM responses for deterministic testing.

use aipack::config::AipackConfig;
use aipack::detection::service::DetectionService;
use aipack::output::schema::UniversalBuild;
use aipack::fs::RealFileSystem;
use aipack::languages::LanguageRegistry;
use aipack::llm::{select_llm_client, RecordingLLMClient, RecordingMode};
use aipack::pipeline::{PipelineConfig, PipelineContext};
use aipack::validation::Validator;
use serial_test::serial;
use std::path::PathBuf;
use std::sync::Arc;

/// Helper to create a detection service with recording enabled
async fn create_detection_service() -> DetectionService {
    let config = AipackConfig::default();

    // Select LLM client (will use embedded if no API keys available)
    let selected = select_llm_client(&config, false)
        .await
        .expect("Failed to select LLM client");

    // Wrap with recording client in Auto mode
    let recordings_dir = PathBuf::from("tests/recordings");
    let recording_client = RecordingLLMClient::new(
        selected.client.clone(),
        RecordingMode::Auto,
        recordings_dir,
    )
    .expect("Failed to create recording client");

    let client = Arc::new(recording_client) as Arc<dyn aipack::llm::LLMClient>;

    // Create pipeline context
    let context = Arc::new(PipelineContext::new(
        client.clone(),
        Arc::new(RealFileSystem),
        Arc::new(LanguageRegistry::with_defaults()),
        Arc::new(Validator::new()),
        PipelineConfig::default(),
    ));

    DetectionService::new(client, context)
}

/// Helper to get fixture path
fn fixture_path(category: &str, name: &str) -> PathBuf {
    PathBuf::from("tests/fixtures").join(category).join(name)
}

/// Helper to load expected UniversalBuild(s) from JSON
/// Returns single UniversalBuild for single-project, first element for multi-project
fn load_expected(fixture_name: &str) -> Option<UniversalBuild> {
    let expected_path = PathBuf::from("tests/fixtures/expected").join(format!("{}.json", fixture_name));

    if !expected_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&expected_path)
        .expect(&format!("Failed to read expected JSON: {}", expected_path.display()));

    // Try parsing as single UniversalBuild first
    if let Ok(single) = serde_json::from_str::<UniversalBuild>(&content) {
        return Some(single);
    }

    // Try parsing as array of UniversalBuild (for monorepos)
    if let Ok(multi) = serde_json::from_str::<Vec<UniversalBuild>>(&content) {
        return multi.into_iter().next();
    }

    panic!("Failed to parse expected JSON as UniversalBuild or Vec<UniversalBuild>: {}", expected_path.display())
}

/// Helper to assert detection results against expected output
fn assert_detection(result: &UniversalBuild, expected_build_system: &str, fixture_name: &str) {
    // Basic assertions
    assert_eq!(
        result.metadata.build_system, expected_build_system,
        "Expected build system '{}', got '{}'",
        expected_build_system, result.metadata.build_system
    );

    assert!(
        !result.build.commands.is_empty(),
        "Build commands should not be empty"
    );

    assert!(
        result.metadata.confidence >= 0.5,
        "Confidence should be at least 0.5, got {}",
        result.metadata.confidence
    );

    // Validate against expected JSON if it exists
    if let Some(expected) = load_expected(fixture_name) {
        assert_eq!(
            result.metadata.language, expected.metadata.language,
            "Language mismatch"
        );
        assert_eq!(
            result.metadata.build_system, expected.metadata.build_system,
            "Build system mismatch"
        );
        assert_eq!(
            result.build.base, expected.build.base,
            "Build base image mismatch"
        );
        assert_eq!(
            result.runtime.base, expected.runtime.base,
            "Runtime base image mismatch"
        );
        // Note: Commands and other fields may vary slightly but core structure should match
    }
}

//
// Single-language tests
//

#[tokio::test]
#[serial]
async fn test_rust_cargo_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "rust-cargo");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "cargo", "rust-cargo");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("cargo build")),
        "Should contain cargo build command"
    );
}

#[tokio::test]
#[serial]
async fn test_node_npm_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "node-npm");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "npm", "node-npm");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("npm")),
        "Should contain npm command"
    );
}

#[tokio::test]
#[serial]
async fn test_python_pip_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "python-pip");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "pip", "python-pip");
}

#[tokio::test]
#[serial]
async fn test_java_maven_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "java-maven");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "maven", "java-maven");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("mvn")),
        "Should contain mvn command"
    );
}

#[tokio::test]
#[serial]
async fn test_go_mod_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "go-mod");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "go", "go-mod");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("go build")),
        "Should contain go build command"
    );
}

#[tokio::test]
#[serial]
async fn test_node_yarn_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "node-yarn");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "yarn", "node-yarn");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("yarn")),
        "Should contain yarn command"
    );
}

#[tokio::test]
#[serial]
async fn test_node_pnpm_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "node-pnpm");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "pnpm", "node-pnpm");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("pnpm")),
        "Should contain pnpm command"
    );
}

#[tokio::test]
#[serial]
async fn test_python_poetry_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "python-poetry");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "poetry", "python-poetry");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("poetry")),
        "Should contain poetry command"
    );
}

#[tokio::test]
#[serial]
async fn test_java_gradle_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "java-gradle");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "gradle", "java-gradle");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("gradle")),
        "Should contain gradle command"
    );
}

#[tokio::test]
#[serial]
async fn test_kotlin_gradle_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "kotlin-gradle");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "gradle", "kotlin-gradle");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("gradle")),
        "Should contain gradle command"
    );
}

#[tokio::test]
#[serial]
async fn test_dotnet_csproj_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "dotnet-csproj");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "dotnet", "dotnet-csproj");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("dotnet")),
        "Should contain dotnet command"
    );
}

#[tokio::test]
#[serial]
async fn test_rust_workspace_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "rust-workspace");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "cargo", "rust-workspace");
    assert!(
        result.build.commands.iter().any(|cmd| cmd.contains("cargo")),
        "Should contain cargo command"
    );
}

//
// Monorepo tests
//

#[tokio::test]
#[serial]
async fn test_npm_workspaces_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("monorepo", "npm-workspaces");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "npm", "npm-workspaces");
    // Monorepo support in Phase 18, for now just check it detects npm
}

#[tokio::test]
#[serial]
async fn test_cargo_workspace_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("monorepo", "cargo-workspace");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "cargo", "cargo-workspace");
    // Monorepo support in Phase 18, for now just check it detects cargo
}

#[tokio::test]
#[serial]
async fn test_turborepo_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("monorepo", "turborepo");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "turborepo", "turborepo");
    // Monorepo support in Phase 18, for now just check it detects turborepo
}

#[tokio::test]
#[serial]
async fn test_gradle_multiproject_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("monorepo", "gradle-multiproject");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "gradle", "gradle-multiproject");
    // Monorepo support in Phase 18, for now just check it detects gradle
}

#[tokio::test]
#[serial]
async fn test_maven_multimodule_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("monorepo", "maven-multimodule");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    assert_detection(&result, "maven", "maven-multimodule");
    // Monorepo support in Phase 18, for now just check it detects maven
}

#[tokio::test]
#[serial]
async fn test_polyglot_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("monorepo", "polyglot");

    let result = service
        .detect(fixture)
        .await
        .expect("Detection failed");

    // Polyglot repos may detect as the primary language
    // Just verify detection works without specifying exact build system
    assert!(
        !result.build.commands.is_empty(),
        "Build commands should not be empty"
    );
    assert!(
        result.metadata.confidence >= 0.5,
        "Confidence should be at least 0.5"
    );
}

//
// Edge case tests
//

#[tokio::test]
#[serial]
async fn test_empty_repo_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("edge-cases", "empty-repo");

    // Empty repo should fail detection gracefully
    let result = service.detect(fixture).await;

    // Should either fail or return very low confidence
    if let Ok(build) = result {
        assert!(
            build.metadata.confidence < 0.3,
            "Empty repo should have low confidence"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_no_manifest_detection() {
    let service = create_detection_service().await;
    let fixture = fixture_path("edge-cases", "no-manifest");

    // No manifest should fail or return low confidence
    let result = service.detect(fixture).await;

    if let Ok(build) = result {
        assert!(
            build.metadata.confidence < 0.5,
            "No manifest should have low confidence"
        );
    }
}

//
// Performance tests
//

#[tokio::test]
#[serial]
async fn test_detection_timeout() {
    let service = create_detection_service().await;
    let fixture = fixture_path("single-language", "rust-cargo");

    let start = std::time::Instant::now();
    let _ = service.detect(fixture).await;
    let elapsed = start.elapsed();

    // Detection should complete within reasonable time (60 seconds)
    // This is generous to account for model loading on first run
    assert!(
        elapsed.as_secs() < 60,
        "Detection took too long: {:?}",
        elapsed
    );
}

use aipack::detection::service::DetectionService;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::Result;
use genai::adapter::AdapterKind;
use tracing::{info, warn};
use aipack::{LanguageRegistry, PipelineConfig, PipelineContext, RealFileSystem, UniversalBuild, Validator};
use aipack::llm::{EmbeddedClient, SelectedClient};

/// Base directory for all test fixtures
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Directory where expected UniversalBuild JSON files are stored
fn expected_dir() -> PathBuf {
    fixtures_dir().join("expected")
}

/// Test a single fixture by comparing detected UniversalBuild with expected output
async fn test_fixture(fixture_path: &Path, fixture_name: &str) -> Result<()> {
    println!("Testing fixture: {}", fixture_name);

    let client = EmbeddedClient::new(false).await?;
    let client_arc = Arc::new(client);

    let context = Arc::new(PipelineContext::new(
        client_arc.clone(),
        Arc::new(RealFileSystem),
        Arc::new(LanguageRegistry::with_defaults()),
        Arc::new(Validator::new()),
        PipelineConfig::default(),
    ));

    // Run detection on the fixture
    let service = DetectionService::new(client_arc, context);
    let detected = service.detect(fixture_path.to_path_buf()).await?;

    // Serialize to pretty JSON
    let detected_json = serde_json::to_string_pretty(&detected)?;

    // Path to expected output file
    let expected_file = expected_dir().join(format!("{}.json", fixture_name));

    if expected_file.exists() {
        // Compare with expected output
        let expected_json = fs::read_to_string(&expected_file)?;
        let expected: UniversalBuild = serde_json::from_str(&expected_json)?;

        // Compare key fields (allowing some flexibility in reasoning text)
        assert_eq!(
            detected.metadata.language,
            expected.metadata.language,
            "Language mismatch for {}",
            fixture_name
        );
        assert_eq!(
            detected.metadata.build_system,
            expected.metadata.build_system,
            "Build system mismatch for {}",
            fixture_name
        );
        assert_eq!(
            detected.build.base,
            expected.build.base,
            "Build base image mismatch for {}",
            fixture_name
        );
        assert_eq!(
            detected.runtime.base,
            expected.runtime.base,
            "Runtime base image mismatch for {}",
            fixture_name
        );

        // Verify commands are not empty
        assert!(
            !detected.build.commands.is_empty(),
            "Build commands should not be empty for {}",
            fixture_name
        );

        println!("✓ Fixture {} matches expected output", fixture_name);
    } else {
        // Generate expected output file
        fs::create_dir_all(&expected_dir())?;
        fs::write(&expected_file, &detected_json)?;
        println!(
            "⚠ Generated expected output for {}: {}",
            fixture_name,
            expected_file.display()
        );
        println!("Please review and commit this file if correct.");
    }

    Ok(())
}

// ============================================================================
// Single-Language Fixture Tests
// ============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --test fixtures_e2e -- --ignored
async fn test_rust_cargo() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("rust-cargo");
    test_fixture(&path, "rust-cargo").await
}

#[tokio::test]
#[ignore]
async fn test_rust_workspace() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("rust-workspace");
    test_fixture(&path, "rust-workspace").await
}

#[tokio::test]
#[ignore]
async fn test_node_npm() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("node-npm");
    test_fixture(&path, "node-npm").await
}

#[tokio::test]
#[ignore]
async fn test_node_yarn() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("node-yarn");
    test_fixture(&path, "node-yarn").await
}

#[tokio::test]
#[ignore]
async fn test_node_pnpm() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("node-pnpm");
    test_fixture(&path, "node-pnpm").await
}

#[tokio::test]
#[ignore]
async fn test_python_pip() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("python-pip");
    test_fixture(&path, "python-pip").await
}

#[tokio::test]
#[ignore]
async fn test_python_poetry() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("python-poetry");
    test_fixture(&path, "python-poetry").await
}

#[tokio::test]
#[ignore]
async fn test_java_maven() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("java-maven");
    test_fixture(&path, "java-maven").await
}

#[tokio::test]
#[ignore]
async fn test_java_gradle() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("java-gradle");
    test_fixture(&path, "java-gradle").await
}

#[tokio::test]
#[ignore]
async fn test_kotlin_gradle() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("kotlin-gradle");
    test_fixture(&path, "kotlin-gradle").await
}

#[tokio::test]
#[ignore]
async fn test_go_mod() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("go-mod");
    test_fixture(&path, "go-mod").await
}

#[tokio::test]
#[ignore]
async fn test_dotnet_csproj() -> Result<()> {
    let path = fixtures_dir().join("single-language").join("dotnet-csproj");
    test_fixture(&path, "dotnet-csproj").await
}

// ============================================================================
// Monorepo Fixture Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_npm_workspaces() -> Result<()> {
    let path = fixtures_dir().join("monorepo").join("npm-workspaces");
    test_fixture(&path, "npm-workspaces").await
}

#[tokio::test]
#[ignore]
async fn test_turborepo() -> Result<()> {
    let path = fixtures_dir().join("monorepo").join("turborepo");
    test_fixture(&path, "turborepo").await
}

#[tokio::test]
#[ignore]
async fn test_cargo_workspace() -> Result<()> {
    let path = fixtures_dir().join("monorepo").join("cargo-workspace");
    test_fixture(&path, "cargo-workspace").await
}

#[tokio::test]
#[ignore]
async fn test_gradle_multiproject() -> Result<()> {
    let path = fixtures_dir().join("monorepo").join("gradle-multiproject");
    test_fixture(&path, "gradle-multiproject").await
}

#[tokio::test]
#[ignore]
async fn test_maven_multimodule() -> Result<()> {
    let path = fixtures_dir().join("monorepo").join("maven-multimodule");
    test_fixture(&path, "maven-multimodule").await
}

#[tokio::test]
#[ignore]
async fn test_polyglot() -> Result<()> {
    let path = fixtures_dir().join("monorepo").join("polyglot");
    test_fixture(&path, "polyglot").await
}

// ============================================================================
// Edge Case Fixture Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_empty_repo() -> Result<()> {
    let path = fixtures_dir().join("edge-cases").join("empty-repo");
    test_fixture(&path, "empty-repo").await
}

#[tokio::test]
#[ignore]
async fn test_no_manifest() -> Result<()> {
    let path = fixtures_dir().join("edge-cases").join("no-manifest");
    test_fixture(&path, "no-manifest").await
}

#[tokio::test]
#[ignore]
async fn test_multiple_manifests() -> Result<()> {
    let path = fixtures_dir().join("edge-cases").join("multiple-manifests");
    test_fixture(&path, "multiple-manifests").await
}

#[tokio::test]
#[ignore]
async fn test_nested_projects() -> Result<()> {
    let path = fixtures_dir().join("edge-cases").join("nested-projects");
    test_fixture(&path, "nested-projects").await
}

#[tokio::test]
#[ignore]
async fn test_vendor_heavy() -> Result<()> {
    let path = fixtures_dir().join("edge-cases").join("vendor-heavy");
    test_fixture(&path, "vendor-heavy").await
}

// ============================================================================
// Batch Test Runner
// ============================================================================

/// Run all fixture tests in sequence and report results
#[tokio::test]
#[ignore]
async fn test_all_fixtures() {
    let fixtures = vec![
        // Single-language
        ("single-language/rust-cargo", "rust-cargo"),
        ("single-language/rust-workspace", "rust-workspace"),
        ("single-language/node-npm", "node-npm"),
        ("single-language/node-yarn", "node-yarn"),
        ("single-language/node-pnpm", "node-pnpm"),
        ("single-language/python-pip", "python-pip"),
        ("single-language/python-poetry", "python-poetry"),
        ("single-language/java-maven", "java-maven"),
        ("single-language/java-gradle", "java-gradle"),
        ("single-language/kotlin-gradle", "kotlin-gradle"),
        ("single-language/go-mod", "go-mod"),
        ("single-language/dotnet-csproj", "dotnet-csproj"),
        // Monorepos
        ("monorepo/npm-workspaces", "npm-workspaces"),
        ("monorepo/turborepo", "turborepo"),
        ("monorepo/cargo-workspace", "cargo-workspace"),
        ("monorepo/gradle-multiproject", "gradle-multiproject"),
        ("monorepo/maven-multimodule", "maven-multimodule"),
        ("monorepo/polyglot", "polyglot"),
        // Edge cases
        ("edge-cases/empty-repo", "empty-repo"),
        ("edge-cases/no-manifest", "no-manifest"),
        ("edge-cases/multiple-manifests", "multiple-manifests"),
        ("edge-cases/nested-projects", "nested-projects"),
        ("edge-cases/vendor-heavy", "vendor-heavy"),
    ];

    let mut passed = 0;
    let mut failed = 0;
    let mut generated = 0;

    for (path_suffix, name) in fixtures {
        let path = fixtures_dir().join(path_suffix);
        print!("Testing {}... ", name);

        match test_fixture(&path, name).await {
            Ok(_) => {
                let expected_file = expected_dir().join(format!("{}.json", name));
                if expected_file.exists() {
                    println!("✓ PASS");
                    passed += 1;
                } else {
                    println!("⚠ GENERATED");
                    generated += 1;
                }
            }
            Err(e) => {
                println!("✗ FAIL: {}", e);
                failed += 1;
            }
        }
    }

    println!("\n========================================");
    println!("Test Results:");
    println!("  Passed:    {}", passed);
    println!("  Failed:    {}", failed);
    println!("  Generated: {}", generated);
    println!("  Total:     {}", passed + failed + generated);
    println!("========================================");

    assert_eq!(failed, 0, "Some fixture tests failed");
}

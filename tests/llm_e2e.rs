//! End-to-end LLM detection tests
//!
//! These tests verify the detection pipeline using LLM (and Full) modes.
//!
//! Tests use RecordingMode::Auto to replay cached LLM responses.

#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::type_complexity)]

mod support;

use serial_test::serial;
use support::e2e::{assert_detection_with_mode, fixture_path, run_detection_with_mode};
use yare::parameterized;

// Single-language fixtures - LLM and Full modes
#[parameterized(
    rust_cargo_full = { "rust-cargo", None },
    rust_cargo_llm = { "rust-cargo", Some("llm") },
    node_npm_full = { "node-npm", None },
    node_npm_llm = { "node-npm", Some("llm") },
    python_pip_full = { "python-pip", None },
    python_pip_llm = { "python-pip", Some("llm") },
    java_maven_full = { "java-maven", None },
    java_maven_llm = { "java-maven", Some("llm") },
    node_yarn_full = { "node-yarn", None },
    node_yarn_llm = { "node-yarn", Some("llm") },
    node_pnpm_full = { "node-pnpm", None },
    node_pnpm_llm = { "node-pnpm", Some("llm") },
    python_poetry_full = { "python-poetry", None },
    python_poetry_llm = { "python-poetry", Some("llm") },
    java_gradle_full = { "java-gradle", None },
    java_gradle_llm = { "java-gradle", Some("llm") },
    kotlin_gradle_full = { "kotlin-gradle", None },
    kotlin_gradle_llm = { "kotlin-gradle", Some("llm") },
    dotnet_csproj_full = { "dotnet-csproj", None },
    dotnet_csproj_llm = { "dotnet-csproj", Some("llm") },
    go_mod_full = { "go-mod", None },
    go_mod_llm = { "go-mod", Some("llm") },
    ruby_bundler_full = { "ruby-bundler", None },
    ruby_bundler_llm = { "ruby-bundler", Some("llm") },
    php_composer_full = { "php-composer", None },
    php_composer_llm = { "php-composer", Some("llm") },
    php_symfony_full = { "php-symfony", None },
    php_symfony_llm = { "php-symfony", Some("llm") },
    cpp_cmake_full = { "cpp-cmake", None },
    cpp_cmake_llm = { "cpp-cmake", Some("llm") },
    elixir_mix_full = { "elixir-mix", None },
    elixir_mix_llm = { "elixir-mix", Some("llm") },
    zig_build_llm = { "zig-build", Some("llm") },
    deno_fresh_llm = { "deno-fresh", Some("llm") },
)]
#[serial]
fn test_single_language(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("single-language", fixture_name);
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture_name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "single-language", fixture_name, mode);
}

// Monorepo fixtures - LLM and Full modes
#[parameterized(
    npm_workspaces_full = { "npm-workspaces", None },
    npm_workspaces_llm = { "npm-workspaces", Some("llm") },
    cargo_workspace_full = { "cargo-workspace", None },
    cargo_workspace_llm = { "cargo-workspace", Some("llm") },
    turborepo_full = { "turborepo", None },
    turborepo_llm = { "turborepo", Some("llm") },
    gradle_multiproject_full = { "gradle-multiproject", None },
    gradle_multiproject_llm = { "gradle-multiproject", Some("llm") },
    maven_multimodule_full = { "maven-multimodule", None },
    maven_multimodule_llm = { "maven-multimodule", Some("llm") },
    polyglot_full = { "polyglot", None },
    polyglot_llm = { "polyglot", Some("llm") },
)]
#[serial]
fn test_monorepo(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("monorepo", fixture_name);
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture_name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "monorepo", fixture_name, mode);
}

// Edge-cases fixtures - LLM mode only for unknown technologies
#[parameterized(
    bazel_build_llm = { "bazel-build", Some("llm") },
)]
#[serial]
fn test_edge_cases(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("edge-cases", fixture_name);
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture_name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "edge-cases", fixture_name, mode);
}

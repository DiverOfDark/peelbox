//! End-to-end Static detection tests
//!
//! These tests verify the detection pipeline using only Static mode.

#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::type_complexity)]

mod support;

use serial_test::serial;
use support::e2e::{assert_detection_with_mode, fixture_path, run_detection_with_mode};
use yare::parameterized;

// Single-language fixtures - Static mode
#[parameterized(
    rust_cargo_static = { "rust-cargo", Some("static") },
    node_npm_static = { "node-npm", Some("static") },
    python_pip_static = { "python-pip", Some("static") },
    java_maven_static = { "java-maven", Some("static") },
    node_yarn_static = { "node-yarn", Some("static") },
    node_pnpm_static = { "node-pnpm", Some("static") },
    python_poetry_static = { "python-poetry", Some("static") },
    java_gradle_static = { "java-gradle", Some("static") },
    kotlin_gradle_static = { "kotlin-gradle", Some("static") },
    dotnet_csproj_static = { "dotnet-csproj", Some("static") },
    go_mod_static = { "go-mod", Some("static") },
    ruby_bundler_static = { "ruby-bundler", Some("static") },
    php_composer_static = { "php-composer", Some("static") },
    php_symfony_static = { "php-symfony", Some("static") },
    cpp_cmake_static = { "cpp-cmake", Some("static") },
    elixir_mix_static = { "elixir-mix", Some("static") },
)]
#[serial]
fn test_single_language(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("single-language", fixture_name);
    let test_name = format!("e2e_test_{}_static", fixture_name.replace("-", "_"));
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "single-language", fixture_name, mode);
}

// Monorepo fixtures - Static mode
#[parameterized(
    npm_workspaces_static = { "npm-workspaces", Some("static") },
    cargo_workspace_static = { "cargo-workspace", Some("static") },
    turborepo_static = { "turborepo", Some("static") },
    gradle_multiproject_static = { "gradle-multiproject", Some("static") },
    maven_multimodule_static = { "maven-multimodule", Some("static") },
    polyglot_static = { "polyglot", Some("static") },
)]
#[serial]
fn test_monorepo(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("monorepo", fixture_name);
    let test_name = format!("e2e_test_{}_static", fixture_name.replace("-", "_"));
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "monorepo", fixture_name, mode);
}

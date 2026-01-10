//! End-to-end Container integration tests
//!
//! These tests verify that the generated images actually run and pass health checks.

#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::type_complexity)]

mod support;

use serial_test::serial;
use support::e2e::run_container_integration_test;
use yare::parameterized;

// Container integration tests for single-language fixtures
// Tests run serially to avoid BuildKit conflicts
#[parameterized(
    rust_cargo = { "rust-cargo" },
    go_mod = { "go-mod" },
    python_pip = { "python-pip" },
    python_poetry = { "python-poetry" },
    node_npm = { "node-npm" },
    ruby_bundler = { "ruby-bundler" },
    java_maven = { "java-maven" },
    java_gradle = { "java-gradle" },
    dotnet_csproj = { "dotnet-csproj" },
    php_symfony = { "php-symfony" },
)]
#[serial]
fn test_container_integration_single_language(fixture_name: &str) {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    runtime.block_on(async {
        run_container_integration_test("single-language", fixture_name)
            .await
            .expect("Container integration test failed");
    });
}

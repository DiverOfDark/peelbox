//! Example: Analyzing a repository with the RepositoryAnalyzer
//!
//! This example demonstrates how to use the RepositoryAnalyzer to:
//! - Walk a repository's file system
//! - Detect key configuration files
//! - Extract README content
//! - Build a comprehensive RepositoryContext
//!
//! Run this example with:
//! ```bash
//! cargo run --example analyze_repository -- /path/to/repo
//! ```

use aipack::detection::analyzer::{AnalyzerConfig, RepositoryAnalyzer};
use std::env;
use std::path::PathBuf;
use std::process;

#[tokio::main]
async fn main() {
    // Get repository path from command line or use current directory
    let repo_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    println!("Analyzing repository: {}", repo_path.display());
    println!("{}", "=".repeat(60));
    println!();

    // Create analyzer with custom configuration
    let config = AnalyzerConfig {
        max_depth: 5,
        file_tree_limit: 200,
        ignore_patterns: {
            let mut patterns = AnalyzerConfig::default_ignores();
            patterns.push(r"^\..*".to_string()); // Ignore hidden files
            patterns
        },
        ..Default::default()
    };

    let analyzer = RepositoryAnalyzer::with_config(repo_path, config);

    // Perform analysis
    match analyzer.analyze().await {
        Ok(context) => {
            println!("Analysis completed successfully!");
            println!();

            // Display file tree
            println!("File Tree:");
            println!("{}", "-".repeat(60));
            println!("{}", context.file_tree);
            println!();

            // Display detected key files
            println!("Detected Key Files ({}):", context.key_file_count());
            println!("{}", "-".repeat(60));
            for (file, content) in &context.key_files {
                println!("  {} ({} bytes)", file, content.len());
            }
            println!();

            // Display README if found
            if let Some(readme) = &context.readme_content {
                println!("README Content:");
                println!("{}", "-".repeat(60));
                let preview = if readme.len() > 500 {
                    format!("{}...", &readme[..500])
                } else {
                    readme.clone()
                };
                println!("{}", preview);
                println!();
            } else {
                println!("No README found");
                println!();
            }

            // Display detected files list
            println!("All Detected Configuration Files:");
            println!("{}", "-".repeat(60));
            for file in &context.detected_files {
                println!("  - {}", file);
            }
            println!();

            // Display repository summary
            println!("Summary:");
            println!("{}", "-".repeat(60));
            println!("  Repository path: {}", context.repo_path.display());
            println!("  Key files: {}", context.key_file_count());
            println!("  Has README: {}", context.readme_content.is_some());
            println!(
                "  Git info: {}",
                if context.git_info.is_some() {
                    "Yes"
                } else {
                    "No"
                }
            );

            // Try to infer build system
            println!();
            println!("Likely Build Systems:");
            println!("{}", "-".repeat(60));
            if context.has_file("Cargo.toml") {
                println!("  - Rust (Cargo)");
            }
            if context.has_file("package.json") {
                println!("  - Node.js (npm/yarn/pnpm)");
            }
            if context.has_file("go.mod") {
                println!("  - Go");
            }
            if context.has_file("pom.xml") {
                println!("  - Java (Maven)");
            }
            if context.has_file("build.gradle") || context.has_file("build.gradle.kts") {
                println!("  - Java/Kotlin (Gradle)");
            }
            if context.has_file("pyproject.toml") || context.has_file("setup.py") {
                println!("  - Python");
            }
            if context.has_file("Makefile") || context.has_file("makefile") {
                println!("  - Make");
            }
            if context.has_file("Dockerfile") {
                println!("  - Docker");
            }
        }
        Err(e) => {
            eprintln!("Error analyzing repository: {}", e);
            eprintln!();

            // Provide helpful error messages
            match e {
                aipack::AnalysisError::PathNotFound(path) => {
                    eprintln!("The path '{}' does not exist.", path.display());
                    eprintln!("Please provide a valid repository path.");
                }
                aipack::AnalysisError::NotADirectory(path) => {
                    eprintln!("The path '{}' is not a directory.", path.display());
                    eprintln!("Please provide a path to a directory, not a file.");
                }
                aipack::AnalysisError::PermissionDenied(msg) => {
                    eprintln!("Permission denied: {}", msg);
                    eprintln!("Please check file permissions and try again.");
                }
                aipack::AnalysisError::TooLarge(limit) => {
                    eprintln!("Repository too large (exceeded {} entries).", limit);
                    eprintln!("Try increasing the file_tree_limit in AnalyzerConfig.");
                }
                _ => {
                    eprintln!("An unexpected error occurred.");
                }
            }

            process::exit(1);
        }
    }
}

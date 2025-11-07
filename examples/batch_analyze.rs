//! Example: Batch Analysis of Multiple Repositories
//!
//! This example demonstrates how to:
//! - Analyze multiple repositories in a single run
//! - Compare detection results across repositories
//! - Generate reports in different formats
//! - Handle errors gracefully during batch processing
//!
//! Run this example with:
//! ```bash
//! # Analyze all subdirectories in a directory
//! cargo run --example batch_analyze -- /path/to/repos/parent
//!
//! # Analyze specific repositories
//! cargo run --example batch_analyze -- /repo1 /repo2 /repo3
//! ```

use aipack::{AipackConfig, DetectionService};
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::Instant;

/// Result of analyzing a single repository
#[derive(Debug, Clone, Serialize)]
struct AnalysisResult {
    path: String,
    success: bool,
    build_system: Option<String>,
    language: Option<String>,
    confidence: Option<f32>,
    build_command: Option<String>,
    test_command: Option<String>,
    processing_time_ms: u64,
    error: Option<String>,
}

/// Summary statistics for batch analysis
#[derive(Debug, Serialize)]
struct BatchSummary {
    total_repos: usize,
    successful: usize,
    failed: usize,
    build_systems: HashMap<String, usize>,
    languages: HashMap<String, usize>,
    average_confidence: f32,
    total_time_ms: u64,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    aipack::init_default();

    println!("=== aipack Batch Analysis Example ===");
    println!();

    // Get repository paths from command line
    let repo_paths: Vec<PathBuf> = env::args().skip(1).map(PathBuf::from).collect();

    if repo_paths.is_empty() {
        eprintln!("Usage: cargo run --example batch_analyze -- <repo1> [repo2] [repo3] ...");
        eprintln!();
        eprintln!("Or provide a parent directory to analyze all subdirectories:");
        eprintln!("  cargo run --example batch_analyze -- /path/to/repos");
        std::process::exit(1);
    }

    // Check if we should scan subdirectories
    let repos_to_analyze = if repo_paths.len() == 1 && repo_paths[0].is_dir() {
        println!("Scanning for repositories in: {}", repo_paths[0].display());
        find_repositories(&repo_paths[0])
    } else {
        repo_paths
    };

    if repos_to_analyze.is_empty() {
        eprintln!("No repositories found to analyze");
        std::process::exit(1);
    }

    println!("Found {} repositories to analyze", repos_to_analyze.len());
    println!();

    // Initialize detection service
    let config = AipackConfig::default();

    let service = match DetectionService::new(&config).await {
        Ok(svc) => svc,
        Err(e) => {
            eprintln!("Failed to initialize service: {}", e);
            eprintln!("{}", e.help_message());
            std::process::exit(1);
        }
    };

    println!("Backend: {}", service.backend_name());
    println!();

    // Analyze all repositories
    let batch_start = Instant::now();
    let results = analyze_repositories(&service, repos_to_analyze).await;
    let total_time = batch_start.elapsed();

    // Generate summary
    let summary = generate_summary(&results, total_time.as_millis() as u64);

    // Display results
    println!();
    println!("=== Analysis Complete ===");
    println!();

    display_results(&results);
    display_summary(&summary);

    // Generate reports
    println!();
    println!("=== Generating Reports ===");
    generate_json_report(&results, &summary);
    generate_csv_report(&results);
    generate_markdown_report(&results, &summary);

    println!();
    println!("Batch analysis completed successfully!");
}

/// Find all repositories in a directory (directories with build config files)
fn find_repositories(parent: &PathBuf) -> Vec<PathBuf> {
    let mut repos = Vec::new();

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && is_repository(&path) {
                repos.push(path);
            }
        }
    }

    repos
}

/// Check if a directory looks like a repository
fn is_repository(path: &PathBuf) -> bool {
    let indicators = [
        "Cargo.toml",
        "package.json",
        "pom.xml",
        "build.gradle",
        "go.mod",
        "requirements.txt",
        "composer.json",
    ];

    indicators.iter().any(|file| path.join(file).exists())
}

/// Analyze multiple repositories
async fn analyze_repositories(
    service: &DetectionService,
    repos: Vec<PathBuf>,
) -> Vec<AnalysisResult> {
    let mut results = Vec::new();

    for (index, repo) in repos.iter().enumerate() {
        println!(
            "[{}/{}] Analyzing: {}",
            index + 1,
            repos.len(),
            repo.display()
        );

        let start = Instant::now();

        match service.detect(repo.clone()).await {
            Ok(detection) => {
                let processing_time = start.elapsed().as_millis() as u64;

                results.push(AnalysisResult {
                    path: repo.display().to_string(),
                    success: true,
                    build_system: Some(detection.build_system.clone()),
                    language: Some(detection.language.clone()),
                    confidence: Some(detection.confidence),
                    build_command: Some(detection.build_command.clone()),
                    test_command: Some(detection.test_command.clone()),
                    processing_time_ms: processing_time,
                    error: None,
                });

                println!(
                    "  ✓ {} ({}) - {:.1}%",
                    detection.build_system,
                    detection.language,
                    detection.confidence * 100.0
                );
            }
            Err(e) => {
                let processing_time = start.elapsed().as_millis() as u64;

                results.push(AnalysisResult {
                    path: repo.display().to_string(),
                    success: false,
                    build_system: None,
                    language: None,
                    confidence: None,
                    build_command: None,
                    test_command: None,
                    processing_time_ms: processing_time,
                    error: Some(e.to_string()),
                });

                println!("  ✗ Error: {}", e);
            }
        }
    }

    results
}

/// Generate summary statistics
fn generate_summary(results: &[AnalysisResult], total_time_ms: u64) -> BatchSummary {
    let total_repos = results.len();
    let successful = results.iter().filter(|r| r.success).count();
    let failed = total_repos - successful;

    let mut build_systems: HashMap<String, usize> = HashMap::new();
    let mut languages: HashMap<String, usize> = HashMap::new();
    let mut confidence_sum = 0.0;
    let mut confidence_count = 0;

    for result in results {
        if let Some(build_system) = &result.build_system {
            *build_systems.entry(build_system.clone()).or_insert(0) += 1;
        }

        if let Some(language) = &result.language {
            *languages.entry(language.clone()).or_insert(0) += 1;
        }

        if let Some(confidence) = result.confidence {
            confidence_sum += confidence;
            confidence_count += 1;
        }
    }

    let average_confidence = if confidence_count > 0 {
        confidence_sum / confidence_count as f32
    } else {
        0.0
    };

    BatchSummary {
        total_repos,
        successful,
        failed,
        build_systems,
        languages,
        average_confidence,
        total_time_ms,
    }
}

/// Display detailed results
fn display_results(results: &[AnalysisResult]) {
    println!("Detailed Results:");
    println!("{}", "-".repeat(80));

    for result in results {
        if result.success {
            println!("✓ {}", result.path);
            println!("  Build System: {}", result.build_system.as_ref().unwrap());
            println!("  Language: {}", result.language.as_ref().unwrap());
            println!("  Confidence: {:.1}%", result.confidence.unwrap() * 100.0);
            println!("  Build: {}", result.build_command.as_ref().unwrap());
            println!("  Test: {}", result.test_command.as_ref().unwrap());
        } else {
            println!("✗ {}", result.path);
            println!("  Error: {}", result.error.as_ref().unwrap());
        }
        println!();
    }
}

/// Display summary statistics
fn display_summary(summary: &BatchSummary) {
    println!("Summary Statistics:");
    println!("{}", "=".repeat(80));
    println!("Total Repositories: {}", summary.total_repos);
    println!(
        "Successful: {} ({:.1}%)",
        summary.successful,
        summary.successful as f64 / summary.total_repos as f64 * 100.0
    );
    println!("Failed: {}", summary.failed);
    println!(
        "Average Confidence: {:.1}%",
        summary.average_confidence * 100.0
    );
    println!(
        "Total Processing Time: {:.2}s",
        summary.total_time_ms as f64 / 1000.0
    );
    println!();

    println!("Build Systems:");
    for (build_system, count) in &summary.build_systems {
        println!("  {}: {}", build_system, count);
    }
    println!();

    println!("Languages:");
    for (language, count) in &summary.languages {
        println!("  {}: {}", language, count);
    }
}

/// Generate JSON report
fn generate_json_report(results: &[AnalysisResult], summary: &BatchSummary) {
    #[derive(Serialize)]
    struct Report<'a> {
        summary: &'a BatchSummary,
        results: &'a [AnalysisResult],
    }

    let report = Report { summary, results };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            std::fs::write("batch_analysis_report.json", json)
                .expect("Failed to write JSON report");
            println!("✓ JSON report: batch_analysis_report.json");
        }
        Err(e) => {
            eprintln!("Failed to generate JSON report: {}", e);
        }
    }
}

/// Generate CSV report
fn generate_csv_report(results: &[AnalysisResult]) {
    let mut csv = String::new();
    csv.push_str("Path,Success,Build System,Language,Confidence,Build Command,Test Command,Time (ms),Error\n");

    for result in results {
        csv.push_str(&format!(
            "\"{}\",{},{},{},{},{},{},{},{}\n",
            result.path,
            result.success,
            result.build_system.as_deref().unwrap_or(""),
            result.language.as_deref().unwrap_or(""),
            result
                .confidence
                .map(|c| format!("{:.2}", c))
                .unwrap_or_default(),
            result.build_command.as_deref().unwrap_or(""),
            result.test_command.as_deref().unwrap_or(""),
            result.processing_time_ms,
            result.error.as_deref().unwrap_or("")
        ));
    }

    std::fs::write("batch_analysis_report.csv", csv).expect("Failed to write CSV report");
    println!("✓ CSV report: batch_analysis_report.csv");
}

/// Generate Markdown report
fn generate_markdown_report(results: &[AnalysisResult], summary: &BatchSummary) {
    let mut md = String::new();

    md.push_str("# Batch Analysis Report\n\n");

    md.push_str("## Summary\n\n");
    md.push_str(&format!(
        "- **Total Repositories**: {}\n",
        summary.total_repos
    ));
    md.push_str(&format!("- **Successful**: {}\n", summary.successful));
    md.push_str(&format!("- **Failed**: {}\n", summary.failed));
    md.push_str(&format!(
        "- **Average Confidence**: {:.1}%\n",
        summary.average_confidence * 100.0
    ));
    md.push_str(&format!(
        "- **Total Time**: {:.2}s\n\n",
        summary.total_time_ms as f64 / 1000.0
    ));

    md.push_str("## Results\n\n");
    md.push_str("| Repository | Build System | Language | Confidence | Build Command |\n");
    md.push_str("|------------|--------------|----------|------------|---------------|\n");

    for result in results {
        if result.success {
            md.push_str(&format!(
                "| {} | {} | {} | {:.1}% | {} |\n",
                result.path,
                result.build_system.as_deref().unwrap(),
                result.language.as_deref().unwrap(),
                result.confidence.unwrap() * 100.0,
                result.build_command.as_deref().unwrap()
            ));
        } else {
            md.push_str(&format!(
                "| {} | ✗ Error | - | - | {} |\n",
                result.path,
                result.error.as_deref().unwrap_or("Unknown error")
            ));
        }
    }

    std::fs::write("batch_analysis_report.md", md).expect("Failed to write Markdown report");
    println!("✓ Markdown report: batch_analysis_report.md");
}

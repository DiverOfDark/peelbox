//! Command-line interface module
//!
//! This module provides the complete CLI interface for aipack, including:
//! - Command definitions using clap derive macros
//! - Output formatting for multiple formats (JSON, YAML, human-readable)
//! - Command handlers that wire together detection services
//!
//! # Usage
//!
//! ```ignore
//! use aipack::cli::{CliArgs, Commands};
//! use clap::Parser;
//!
//! let args = CliArgs::parse();
//! match args.command {
//!     Commands::Detect { .. } => { /* handle detect */ }
//!     Commands::Health { .. } => { /* handle health */ }
//! }
//! ```

pub mod commands;
pub mod output;

// Re-export for convenient access
pub use commands::{CliArgs, Commands, DetectArgs, HealthArgs};
pub use output::{OutputFormat, OutputFormatter};

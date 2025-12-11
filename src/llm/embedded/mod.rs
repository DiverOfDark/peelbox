//! Embedded LLM client for zero-config local inference
//!
//! This module provides local LLM inference using Candle, enabling aipack to work
//! without external API keys or Ollama. It automatically detects available hardware
//! (CPU, CUDA, Metal) and selects an appropriate model based on available RAM.

mod client;
mod download;
mod hardware;
mod models;

pub use client::EmbeddedClient;
pub use download::ModelDownloader;
pub use hardware::{ComputeDevice, HardwareCapabilities, HardwareDetector};
pub use models::{EmbeddedModel, ModelSelector};

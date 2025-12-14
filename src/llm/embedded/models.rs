//! Model selection based on hardware capabilities

use super::hardware::HardwareCapabilities;
use tracing::{debug, info, warn};

/// Model file format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    /// GGUF quantized format (smaller, faster loading)
    Gguf,
}

/// Supported embedded models with their requirements
#[derive(Debug, Clone)]
pub struct EmbeddedModel {
    /// Model identifier on HuggingFace
    pub repo_id: &'static str,
    /// Model filename (or pattern for sharded models)
    pub filename: &'static str,
    /// Tokenizer repo (usually same as model)
    pub tokenizer_repo: &'static str,
    /// Approximate RAM required in GB
    pub ram_required_gb: f64,
    /// Human-readable name
    pub display_name: &'static str,
    /// Model parameter count (for display)
    pub params: &'static str,
    /// Whether this model supports tool calling
    pub supports_tools: bool,
    /// Model file format
    pub format: ModelFormat,
}

impl EmbeddedModel {
    /// Qwen2.5-Coder 1.5B - GGUF Q4 quantized (~1.5GB)
    pub const QWEN_1_5B_GGUF: EmbeddedModel = EmbeddedModel {
        repo_id: "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF",
        filename: "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        tokenizer_repo: "Qwen/Qwen2.5-Coder-1.5B-Instruct",
        ram_required_gb: 2.5,
        display_name: "Qwen2.5-Coder 1.5B GGUF",
        params: "1.5B",
        supports_tools: true,
        format: ModelFormat::Gguf,
    };

    /// Qwen2.5-Coder 3B - GGUF Q4 quantized (~3GB)
    pub const QWEN_3B_GGUF: EmbeddedModel = EmbeddedModel {
        repo_id: "Qwen/Qwen2.5-Coder-3Bы-Instruct-GGUF",
        filename: "qwen2.5-coder-3b-instruct-q4_k_m.gguf",
        tokenizer_repo: "Qwen/Qwen2.5-Coder-3B-Instruct",
        ram_required_gb: 4.0,
        display_name: "Qwen2.5-Coder 3B GGUF",
        params: "3B",
        supports_tools: true,
        format: ModelFormat::Gguf,
    };

    /// Qwen2.5-Coder 7B - GGUF Q4 quantized (~5GB)
    pub const QWEN_7B_GGUF: EmbeddedModel = EmbeddedModel {
        repo_id: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
        filename: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        tokenizer_repo: "Qwen/Qwen2.5-Coder-7B-Instruct",
        ram_required_gb: 5.5,
        display_name: "Qwen2.5-Coder 7B GGUF",
        params: "7B",
        supports_tools: true,
        format: ModelFormat::Gguf,
    };

    /// All available models in order of preference (largest first)
    pub const ALL_MODELS: &'static [EmbeddedModel] =
        &[Self::QWEN_7B_GGUF, Self::QWEN_3B_GGUF, Self::QWEN_1_5B_GGUF];
}

/// Selects the best model based on hardware capabilities
pub struct ModelSelector;

impl ModelSelector {
    /// Select the best model that fits in available RAM
    ///
    /// Returns None if no model fits (less than 1GB RAM available)
    pub fn select(capabilities: &HardwareCapabilities) -> Option<&'static EmbeddedModel> {
        // Check for explicit model size override
        if let Ok(model_size) = std::env::var("AIPACK_MODEL_SIZE") {
            info!(
                "AIPACK_MODEL_SIZE={} specified, using explicit model selection",
                model_size
            );

            if let Some(model) = EmbeddedModel::ALL_MODELS
                .iter()
                .find(|m| m.params == model_size)
            {
                info!(
                    "Selected model: {} ({} params, requires {:.1}GB RAM)",
                    model.display_name, model.params, model.ram_required_gb
                );

                // Warn if model might not fit in RAM
                let available_gb = capabilities.available_ram_gb();
                let system_reserve_gb = (available_gb * 0.25).max(2.0);
                let usable_gb = (available_gb - system_reserve_gb).max(0.0);

                if model.ram_required_gb > usable_gb {
                    warn!(
                        "Warning: Model requires {:.1}GB but only {:.1}GB available (after reserves). May cause OOM!",
                        model.ram_required_gb, usable_gb
                    );
                }

                return Some(model);
            } else {
                warn!(
                    "Model size '{}' not found. Available sizes: 0.5B, 1.5B, 3B, 7B. Falling back to auto-selection.",
                    model_size
                );
            }
        }

        let available_gb = capabilities.available_ram_gb();

        // Reserve some RAM for the system (at least 2GB or 25% of available)
        let system_reserve_gb = (available_gb * 0.25).max(2.0);
        let usable_gb = (available_gb - system_reserve_gb).max(0.0);

        debug!(
            "Model selection: {:.1}GB available, {:.1}GB reserved, {:.1}GB usable for model",
            available_gb, system_reserve_gb, usable_gb
        );

        // Find the largest model that fits
        let selected = EmbeddedModel::ALL_MODELS
            .iter()
            .find(|model| model.ram_required_gb <= usable_gb);

        if let Some(model) = selected {
            info!(
                "Selected model: {} ({} params, requires {:.1}GB RAM)",
                model.display_name, model.params, model.ram_required_gb
            );
        } else {
            info!(
                "No suitable model found for {:.1}GB usable RAM (minimum 1GB required)",
                usable_gb
            );
        }

        selected
    }

    /// Get a specific model by parameter count
    pub fn get_model(params: &str) -> Option<&'static EmbeddedModel> {
        EmbeddedModel::ALL_MODELS
            .iter()
            .find(|m| m.params == params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_caps(available_gb: f64, total_gb: f64) -> HardwareCapabilities {
        HardwareCapabilities {
            total_ram_bytes: (total_gb * 1024.0 * 1024.0 * 1024.0) as u64,
            available_ram_bytes: (available_gb * 1024.0 * 1024.0 * 1024.0) as u64,
            cuda_available: false,
            cuda_memory_bytes: None,
            metal_available: false,
            cpu_cores: 8,
        }
    }

    #[test]
    fn test_select_7b_with_plenty_ram() {
        // 7B GGUF requires 5.5GB, plus 25% reserve -> need ~7.5GB available
        let caps = make_caps(10.0, 16.0);
        let model = ModelSelector::select(&caps);
        assert!(model.is_some());
        assert_eq!(model.unwrap().params, "7B");
    }

    #[test]
    fn test_select_3b_with_moderate_ram() {
        // 3B GGUF requires 4GB, plus 2GB reserve -> need ~6GB available
        let caps = make_caps(7.0, 12.0);
        let model = ModelSelector::select(&caps);
        assert!(model.is_some());
        assert_eq!(model.unwrap().params, "3B");
    }

    #[test]
    fn test_select_1_5b_with_limited_ram() {
        // 1.5B GGUF requires 2.5GB, plus 2GB reserve -> need 4.5GB available
        let caps = make_caps(5.0, 8.0);
        let model = ModelSelector::select(&caps);
        assert!(model.is_some());
        assert_eq!(model.unwrap().params, "1.5B");
    }

    #[test]
    fn test_no_model_with_insufficient_ram() {
        // Even smallest model (1.5B GGUF) needs 2GB usable RAM after reserves
        let caps = make_caps(2.0, 3.0);
        let model = ModelSelector::select(&caps);
        assert!(model.is_none());
    }

    #[test]
    fn test_get_model_by_params() {
        assert!(ModelSelector::get_model("7B").is_some());
        assert!(ModelSelector::get_model("3B").is_some());
        assert!(ModelSelector::get_model("1.5B").is_some());
        assert!(ModelSelector::get_model("100B").is_none());
    }
}

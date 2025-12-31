//! Model downloading from HuggingFace Hub

use super::models::EmbeddedModel;
use anyhow::{Context, Result};
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use tracing::{debug, info};

/// Downloads and manages model files from HuggingFace Hub
pub struct ModelDownloader {
    api: Api,
    cache_dir: PathBuf,
}

impl ModelDownloader {
    /// Creates a new downloader using the default HuggingFace cache
    pub fn new() -> Result<Self> {
        let api = Api::new().context("Failed to initialize HuggingFace Hub API")?;

        // Use standard HuggingFace cache location
        let cache_dir = dirs::cache_dir()
            .map(|d| d.join("huggingface").join("hub"))
            .unwrap_or_else(|| PathBuf::from(".cache/huggingface/hub"));

        debug!("HuggingFace cache directory: {}", cache_dir.display());

        Ok(Self { api, cache_dir })
    }

    /// Creates a downloader with a custom cache directory
    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir).context("Failed to create model cache directory")?;

        let api = Api::new().context("Failed to initialize HuggingFace Hub API")?;

        Ok(Self { api, cache_dir })
    }

    /// Returns the cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Check if a model is already downloaded
    pub fn is_downloaded(&self, model: &EmbeddedModel) -> bool {
        self.model_path(model).is_some()
    }

    /// Get the local path to a downloaded model, if it exists
    pub fn model_path(&self, model: &EmbeddedModel) -> Option<PathBuf> {
        let repo = self
            .api
            .repo(Repo::new(model.repo_id.to_string(), RepoType::Model));

        // Try to get the file without downloading
        match repo.get(model.filename) {
            Ok(path) => {
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Download a model and its tokenizer, showing progress if interactive
    ///
    /// If `interactive` is true and stdin is a terminal, prompts for confirmation
    /// before downloading. Returns a list of model file paths (may be multiple for sharded models).
    pub fn download(&self, model: &EmbeddedModel, interactive: bool) -> Result<Vec<PathBuf>> {
        // Check if model already downloaded
        let model_paths = if let Some(paths) = self.model_paths(model) {
            info!(
                "Model {} already downloaded ({} files)",
                model.display_name,
                paths.len()
            );
            paths
        } else {
            // Prompt for confirmation if interactive
            if interactive && std::io::stdin().is_terminal() && !Self::prompt_download(model)? {
                anyhow::bail!("Model download cancelled by user");
            }

            info!(
                "Downloading {} ({} params) from {}...",
                model.display_name, model.params, model.repo_id
            );

            let repo = self
                .api
                .repo(Repo::new(model.repo_id.to_string(), RepoType::Model));

            // Check if model is sharded by looking for index file
            let files_to_download = self.get_model_files(model)?;

            let mut paths = Vec::new();
            for filename in &files_to_download {
                debug!("Downloading: {}", filename);
                let path = repo
                    .get(filename)
                    .context(format!("Failed to download model file: {}", filename))?;
                paths.push(path);
            }

            info!("Model downloaded ({} files)", paths.len());
            paths
        };

        // Download tokenizer (GGUF models still use external tokenizer.json)
        self.ensure_tokenizer(model)?;

        Ok(model_paths)
    }

    /// Get the list of model files to download
    fn get_model_files(&self, model: &EmbeddedModel) -> Result<Vec<String>> {
        // GGUF models are always single files
        debug!("GGUF model, single file: {}", model.filename);
        Ok(vec![model.filename.to_string()])
    }

    /// Get cached model paths if already downloaded
    fn model_paths(&self, model: &EmbeddedModel) -> Option<Vec<PathBuf>> {
        let repo = self
            .api
            .repo(Repo::new(model.repo_id.to_string(), RepoType::Model));

        // Get list of files to check
        let files = self.get_model_files(model).ok()?;

        let mut paths = Vec::new();
        for filename in &files {
            match repo.get(filename) {
                Ok(path) if path.exists() => paths.push(path),
                _ => return None, // If any file is missing, need full download
            }
        }

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }

    /// Ensure the tokenizer is downloaded for a model
    fn ensure_tokenizer(&self, model: &EmbeddedModel) -> Result<PathBuf> {
        // Check if tokenizer already exists
        if let Some(path) = self.tokenizer_path(model) {
            debug!("Tokenizer already downloaded at {}", path.display());
            return Ok(path);
        }

        // Download it
        self.download_tokenizer(model)
    }

    /// Download the tokenizer for a model
    fn download_tokenizer(&self, model: &EmbeddedModel) -> Result<PathBuf> {
        info!("Downloading tokenizer from {}", model.tokenizer_repo);

        let repo = self
            .api
            .repo(Repo::new(model.tokenizer_repo.to_string(), RepoType::Model));

        // Download tokenizer.json
        let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
            anyhow::anyhow!(
                "Failed to download tokenizer.json from {}: {}",
                model.tokenizer_repo,
                e
            )
        })?;

        info!("Tokenizer downloaded to: {}", tokenizer_path.display());

        Ok(tokenizer_path)
    }

    /// Get the tokenizer path for a model
    pub fn tokenizer_path(&self, model: &EmbeddedModel) -> Option<PathBuf> {
        let repo = self
            .api
            .repo(Repo::new(model.tokenizer_repo.to_string(), RepoType::Model));

        match repo.get("tokenizer.json") {
            Ok(path) if path.exists() => Some(path),
            _ => None,
        }
    }

    /// Prompt the user to confirm model download
    fn prompt_download(model: &EmbeddedModel) -> Result<bool> {
        println!();
        println!("peelbox needs to download an embedded LLM model for local inference.");
        println!();
        println!(
            "  Model: {} ({} parameters)",
            model.display_name, model.params
        );
        println!("  Requires: ~{:.1} GB RAM", model.ram_required_gb);
        println!("  Source: huggingface.co/{}", model.repo_id);
        println!();
        print!("Download model? [Y/n] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        Ok(input.is_empty() || input == "y" || input == "yes")
    }

    /// Download without prompting (for CI/non-interactive use)
    pub fn download_silent(&self, model: &EmbeddedModel) -> Result<Vec<PathBuf>> {
        self.download(model, false)
    }
}

impl Default for ModelDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default ModelDownloader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_creation() {
        // This test just verifies the API can be initialized
        // Actual download tests would require network access
        let result = ModelDownloader::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_dir() {
        let downloader = ModelDownloader::new().unwrap();
        let cache_dir = downloader.cache_dir();
        // Cache dir should be set (we don't check if it exists as it may be created lazily)
        assert!(!cache_dir.as_os_str().is_empty());
    }
}

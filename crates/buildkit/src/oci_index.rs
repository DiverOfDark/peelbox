use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

const OCI_INDEX_SCHEMA_VERSION: i32 = 2;
const OCI_INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";
const OCI_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
const ANNOTATION_REF_NAME: &str = "org.opencontainers.image.ref.name";
const DEFAULT_TAG: &str = "latest";
const MAX_MANIFEST_SIZE: i64 = 100_000; // 100KB

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciIndex {
    pub schema_version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    pub manifests: Vec<OciDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    pub digest: String,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl OciIndex {
    /// Get index filename based on optional cache key
    pub fn filename(cache_key: Option<&str>) -> String {
        match cache_key {
            Some(key) => format!("{}.json", key),
            None => "index.json".to_string(),
        }
    }

    /// Create a new empty OCI index
    pub fn new() -> Self {
        Self {
            schema_version: OCI_INDEX_SCHEMA_VERSION,
            media_type: Some(OCI_INDEX_MEDIA_TYPE.to_string()),
            manifests: Vec::new(),
        }
    }

    /// Read index file from cache directory with optional cache key
    pub fn read_with_key(cache_dir: &Path, cache_key: Option<&str>) -> Result<Self> {
        let filename = Self::filename(cache_key);
        Self::read_from_file(&cache_dir.join(filename))
    }

    /// Write index file to cache directory with optional cache key
    pub fn write_with_key(&self, cache_dir: &Path, cache_key: Option<&str>) -> Result<()> {
        let filename = Self::filename(cache_key);
        self.write_to_file(&cache_dir.join(filename))
    }

    /// Read index file from a specific path
    pub fn read_from_file(index_path: &Path) -> Result<Self> {
        if !index_path.exists() {
            debug!(
                "No index file found at {}, creating new",
                index_path.display()
            );
            return Ok(Self::new());
        }

        let content = fs::read_to_string(index_path)
            .with_context(|| format!("Failed to read index file from {}", index_path.display()))?;

        let index: OciIndex = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse index file from {}", index_path.display()))?;

        debug!("Read index file with {} manifests", index.manifests.len());
        Ok(index)
    }

    /// Write index to a specific file path
    pub fn write_to_file(&self, index_path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self).context("Failed to serialize index")?;

        fs::write(index_path, content)
            .with_context(|| format!("Failed to write index to {}", index_path.display()))?;

        info!(
            "Wrote index with {} manifests to {}",
            self.manifests.len(),
            index_path.display()
        );
        Ok(())
    }

    /// Add or update a manifest with a specific tag
    pub fn add_or_update_manifest(&mut self, digest: String, size: i64, tag: &str) {
        // Remove existing manifest with the same tag
        self.manifests.retain(|m| {
            m.annotations
                .as_ref()
                .and_then(|a| a.get(ANNOTATION_REF_NAME))
                .map(|t| t != tag)
                .unwrap_or(true)
        });

        // Add new manifest
        let annotations = HashMap::from([(ANNOTATION_REF_NAME.to_string(), tag.to_string())]);

        self.manifests.push(OciDescriptor {
            media_type: Some(OCI_MANIFEST_MEDIA_TYPE.to_string()),
            digest,
            size,
            annotations: Some(annotations),
        });

        debug!("Added manifest to index with tag '{}'", tag);
    }

    /// Get the digest for a specific tag (defaults to "latest")
    pub fn get_digest(&self, tag: Option<&str>) -> Option<String> {
        let tag = tag.unwrap_or(DEFAULT_TAG);

        self.manifests
            .iter()
            .find(|m| {
                m.annotations
                    .as_ref()
                    .and_then(|a| a.get(ANNOTATION_REF_NAME))
                    .map(|t| t == tag)
                    .unwrap_or(false)
            })
            .map(|m| m.digest.clone())
    }
}

impl Default for OciIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the most recently written OCI image manifest in the cache directory
/// Returns (digest, size) if found
pub fn find_latest_manifest(cache_dir: &Path) -> Result<Option<(String, i64)>> {
    let blobs_dir = cache_dir.join("blobs/sha256");
    if !blobs_dir.exists() {
        return Ok(None);
    }

    let mut manifests = Vec::new();

    for entry in fs::read_dir(&blobs_dir).context("Failed to read blobs directory")? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let metadata = entry.metadata()?;
        let size = metadata.len() as i64;

        if size > MAX_MANIFEST_SIZE {
            continue;
        }

        // Try to read and parse as JSON, check if it's an OCI manifest
        if let Some((digest, modified)) = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
            .filter(|json| json.get("config").is_some() && json.get("layers").is_some())
            .and_then(|_| {
                path.file_name()
                    .map(|name| format!("sha256:{}", name.to_string_lossy()))
                    .zip(metadata.modified().ok())
            })
        {
            debug!("Found OCI manifest candidate: {} (size={})", digest, size);
            manifests.push((digest, size, modified));
        }
    }

    if manifests.is_empty() {
        return Ok(None);
    }

    // Sort by modification time (most recent first)
    manifests.sort_by(|a, b| b.2.cmp(&a.2));

    let (digest, size, _) = &manifests[0];
    info!("Selected latest manifest: {} (size={})", digest, size);
    Ok(Some((digest.clone(), *size)))
}

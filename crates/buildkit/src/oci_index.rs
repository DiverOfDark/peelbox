use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info, warn};

const OCI_INDEX_SCHEMA_VERSION: i32 = 2;
const OCI_INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";
const OCI_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
const ANNOTATION_REF_NAME: &str = "org.opencontainers.image.ref.name";
const DEFAULT_TAG: &str = "latest";
const ANNOTATION_APPLICATION: &str = "pro.kirillorlov.peelbox.application";
const ANNOTATION_UNIVERSAL_BUILD_PATH: &str = "pro.kirillorlov.peelbox.universalbuildpath";
const MAX_MANIFEST_SIZE: i64 = 100_000;

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
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,
}

impl OciIndex {
    pub fn filename(_cache_key: Option<&str>) -> String {
        "index.json".to_string()
    }

    pub fn update_with_lock<F>(
        cache_dir: &Path,
        _cache_key: Option<&str>,
        update_fn: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut OciIndex),
    {
        let filename = "index.json".to_string();
        let index_path = cache_dir.join(&filename);

        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&index_path)
            .with_context(|| format!("Failed to open index file {}", index_path.display()))?;

        file.lock_exclusive()
            .with_context(|| format!("Failed to lock {}", index_path.display()))?;

        use std::io::Read;
        let mut content = String::new();

        let mut file = file;
        let mut index = match file.read_to_string(&mut content) {
            Ok(_) if !content.is_empty() => serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse index from {}", index_path.display()))?,
            _ => Self::new(),
        };

        update_fn(&mut index);

        index.sort_manifests();

        file.set_len(0).context("Failed to truncate index file")?;

        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(0))?;

        let json_value =
            serde_json::to_value(&index).context("Failed to convert index to value")?;
        let content =
            serde_json::to_string_pretty(&json_value).context("Failed to serialize index")?;
        use std::io::Write;
        file.write_all(content.as_bytes())
            .with_context(|| format!("Failed to write index to {}", index_path.display()))?;

        info!(
            "Updated index {} with {} manifests",
            index_path.display(),
            index.manifests.len()
        );

        Ok(())
    }

    pub fn read_with_lock(cache_dir: &Path) -> Result<Self> {
        let filename = "index.json".to_string();
        let index_path = cache_dir.join(&filename);

        if !index_path.exists() {
            return Ok(Self::new());
        }

        let mut file = fs::File::open(&index_path)
            .with_context(|| format!("Failed to open index file {}", index_path.display()))?;

        file.lock_shared()
            .with_context(|| format!("Failed to share-lock {}", index_path.display()))?;

        use std::io::Read;
        let mut content = String::new();
        let index = match file.read_to_string(&mut content) {
            Ok(_) if !content.is_empty() => serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse index from {}", index_path.display()))?,
            _ => Self::new(),
        };

        Ok(index)
    }

    pub fn new() -> Self {
        Self {
            schema_version: OCI_INDEX_SCHEMA_VERSION,
            media_type: Some(OCI_INDEX_MEDIA_TYPE.to_string()),
            manifests: Vec::new(),
        }
    }

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

    pub fn write_to_file(&self, index_path: &Path) -> Result<()> {
        let json_value = serde_json::to_value(self).context("Failed to convert index to value")?;
        let content =
            serde_json::to_string_pretty(&json_value).context("Failed to serialize index")?;

        fs::write(index_path, content)
            .with_context(|| format!("Failed to write index to {}", index_path.display()))?;

        info!(
            "Wrote index with {} manifests to {}",
            self.manifests.len(),
            index_path.display()
        );
        Ok(())
    }

    pub fn add_or_update_manifest(
        &mut self,
        digest: String,
        size: u64,
        tag: &str,
        application: &str,
        universal_build_path: &str,
    ) {
        self.manifests.retain(|m| {
            let matches_tag = m
                .annotations
                .as_ref()
                .and_then(|a| a.get(ANNOTATION_REF_NAME))
                .map(|t| t == tag)
                .unwrap_or(false);

            let matches_app = m
                .annotations
                .as_ref()
                .and_then(|a| a.get(ANNOTATION_APPLICATION))
                .map(|a| a == application)
                .unwrap_or(false);

            let matches_universal_build_path = m
                .annotations
                .as_ref()
                .and_then(|a| a.get(ANNOTATION_UNIVERSAL_BUILD_PATH))
                .map(|a| a == universal_build_path)
                .unwrap_or(false);

            !(matches_tag && matches_app && matches_universal_build_path)
        });

        let annotations = BTreeMap::from([
            (ANNOTATION_REF_NAME.to_string(), tag.to_string()),
            (ANNOTATION_APPLICATION.to_string(), application.to_string()),
            (
                ANNOTATION_UNIVERSAL_BUILD_PATH.to_string(),
                universal_build_path.to_string(),
            ),
        ]);

        self.manifests.push(OciDescriptor {
            media_type: Some(OCI_MANIFEST_MEDIA_TYPE.to_string()),
            digest,
            size,
            annotations: Some(annotations),
        });

        debug!(
            "Added manifest to index with tag '{}' (application: {})",
            tag, application
        );
    }

    pub fn get_digest(
        &self,
        tag: Option<&str>,
        application: &str,
        universal_build_path: &str,
    ) -> Option<String> {
        let tag = tag.unwrap_or(DEFAULT_TAG);

        self.manifests
            .iter()
            .find(|m| {
                let matches_tag = m
                    .annotations
                    .as_ref()
                    .and_then(|a| a.get(ANNOTATION_REF_NAME))
                    .map(|t| t == tag)
                    .unwrap_or(false);

                let matches_app = m
                    .annotations
                    .as_ref()
                    .and_then(|a| a.get(ANNOTATION_APPLICATION))
                    .map(|a| a == application)
                    .unwrap_or(false);

                let matches_universal_build_path = m
                    .annotations
                    .as_ref()
                    .and_then(|a| a.get(ANNOTATION_UNIVERSAL_BUILD_PATH))
                    .map(|a| a == universal_build_path)
                    .unwrap_or(false);

                matches_tag && matches_app && matches_universal_build_path
            })
            .map(|m| m.digest.clone())
    }

    pub fn sort_manifests(&mut self) {
        self.manifests.sort_by(|a, b| {
            let app_a = a
                .annotations
                .as_ref()
                .and_then(|m| m.get(ANNOTATION_APPLICATION));
            let app_b = b
                .annotations
                .as_ref()
                .and_then(|m| m.get(ANNOTATION_APPLICATION));

            let ref_a = a
                .annotations
                .as_ref()
                .and_then(|m| m.get(ANNOTATION_REF_NAME));
            let ref_b = b
                .annotations
                .as_ref()
                .and_then(|m| m.get(ANNOTATION_REF_NAME));

            app_a
                .cmp(&app_b)
                .then(ref_a.cmp(&ref_b))
                .then(a.digest.cmp(&b.digest))
        });
    }

    pub fn get_reachable_digests(&self, cache_dir: &Path) -> HashSet<String> {
        let mut reachable = HashSet::new();

        for manifest_desc in &self.manifests {
            reachable.insert(manifest_desc.digest.clone());

            let blob_path = crate::digest::blob_path_or_fallback(&manifest_desc.digest, cache_dir);
            if let Ok(content) = fs::read_to_string(&blob_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(config_digest) = json.get("config").and_then(|c| c.get("digest")) {
                        if let Some(digest_str) = config_digest.as_str() {
                            reachable.insert(digest_str.to_string());
                        }
                    }

                    if let Some(layers) = json.get("layers").and_then(|l| l.as_array()) {
                        for layer in layers {
                            if let Some(layer_digest) = layer.get("digest").and_then(|d| d.as_str())
                            {
                                reachable.insert(layer_digest.to_string());
                            }
                        }
                    }
                }
            }
        }

        reachable
    }

    pub fn gc(cache_dir: &Path, keep_digests: &HashSet<String>) -> Result<()> {
        let blobs_dir = cache_dir.join("blobs/sha256");
        if !blobs_dir.exists() {
            return Ok(());
        }

        info!("Starting OCI cache garbage collection...");
        let mut deleted_count = 0;
        let mut deleted_size = 0;

        for entry in fs::read_dir(&blobs_dir).context("Failed to read blobs directory")? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let digest = format!("sha256:{}", file_name);

            if !keep_digests.contains(&digest) {
                if let Ok(metadata) = entry.metadata() {
                    deleted_size += metadata.len();
                }
                if let Err(e) = fs::remove_file(&path) {
                    warn!("Failed to delete unreferenced blob {}: {}", digest, e);
                } else {
                    deleted_count += 1;
                    debug!("Deleted unreferenced blob: {}", digest);
                }
            }
        }

        let ingest_dir = cache_dir.join("ingest");
        if ingest_dir.exists() {
            let now = std::time::SystemTime::now();
            let max_age = Duration::from_secs(24 * 3600);

            for entry in fs::read_dir(&ingest_dir).context("Failed to read ingest directory")? {
                let entry = entry?;
                let path = entry.path();

                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age > max_age {
                                if let Err(e) = fs::remove_file(&path) {
                                    warn!(
                                        "Failed to delete old ingest file {}: {}",
                                        path.display(),
                                        e
                                    );
                                } else {
                                    debug!("Deleted old ingest file: {}", path.display());
                                }
                            }
                        }
                    }
                }
            }
        }

        if deleted_count > 0 {
            info!(
                "OCI GC complete: deleted {} blobs, reclaimed {} bytes",
                deleted_count, deleted_size
            );
        } else {
            debug!("OCI GC complete: no unreferenced blobs found");
        }

        Ok(())
    }
}

impl Default for OciIndex {
    fn default() -> Self {
        Self::new()
    }
}

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

    manifests.sort_by(|a, b| b.2.cmp(&a.2));

    let (digest, size, _) = &manifests[0];
    info!("Selected latest manifest: {} (size={})", digest, size);
    Ok(Some((digest.clone(), *size)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_oci_index_reachability_and_gc() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path();
        let blobs_dir = cache_dir.join("blobs/sha256");
        fs::create_dir_all(&blobs_dir).unwrap();

        let layer1_digest = "sha256:layer1hash";
        let layer2_digest = "sha256:layer2hash";
        let config_digest = "sha256:confighash";
        let manifest_digest = "sha256:manifesthash";
        let unreferenced_digest = "sha256:unreferencedhash";

        fs::write(blobs_dir.join("layer1hash"), "layer1 data").unwrap();
        fs::write(blobs_dir.join("layer2hash"), "layer2 data").unwrap();
        fs::write(blobs_dir.join("confighash"), "config data").unwrap();
        fs::write(blobs_dir.join("unreferencedhash"), "unreferenced data").unwrap();

        let manifest_content = serde_json::json!({
            "config": { "digest": config_digest },
            "layers": [
                { "digest": layer1_digest },
                { "digest": layer2_digest }
            ]
        });
        fs::write(blobs_dir.join("manifesthash"), manifest_content.to_string()).unwrap();

        let mut index = OciIndex::new();
        index.add_or_update_manifest(
            manifest_digest.to_string(),
            100,
            "latest",
            "test-app",
            "spec.json",
        );

        let reachable = index.get_reachable_digests(cache_dir);
        assert!(reachable.contains(manifest_digest));
        assert!(reachable.contains(config_digest));
        assert!(reachable.contains(layer1_digest));
        assert!(reachable.contains(layer2_digest));
        assert!(!reachable.contains(unreferenced_digest));

        OciIndex::gc(cache_dir, &reachable).unwrap();

        assert!(blobs_dir.join("manifesthash").exists());
        assert!(blobs_dir.join("confighash").exists());
        assert!(blobs_dir.join("layer1hash").exists());
        assert!(blobs_dir.join("layer2hash").exists());
        assert!(!blobs_dir.join("unreferencedhash").exists());
    }
}

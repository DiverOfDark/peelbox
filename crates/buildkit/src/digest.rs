use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Represents a content-addressable digest (e.g., "sha256:abc123...")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    algorithm: String,
    hash: String,
}

impl Digest {
    /// Parse a digest string in format "algorithm:hash"
    pub fn parse(digest: &str) -> Result<Self> {
        let (algorithm, hash) = digest.split_once(':').with_context(|| {
            format!(
                "Invalid digest format (expected 'algorithm:hash'): {}",
                digest
            )
        })?;

        Ok(Self {
            algorithm: algorithm.to_string(),
            hash: hash.to_string(),
        })
    }

    /// Get the algorithm part (e.g., "sha256")
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// Get the hash part
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Convert digest to blob storage path within a cache directory
    pub fn to_blob_path(&self, cache_dir: &Path) -> PathBuf {
        cache_dir
            .join("blobs")
            .join(&self.algorithm)
            .join(&self.hash)
    }

    /// Format as "algorithm:hash" string
    pub fn as_str(&self) -> String {
        format!("{}:{}", self.algorithm, self.hash)
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_digest() {
        let digest = Digest::parse("sha256:abc123").unwrap();
        assert_eq!(digest.algorithm(), "sha256");
        assert_eq!(digest.hash(), "abc123");
        assert_eq!(digest.to_string(), "sha256:abc123");
    }

    #[test]
    fn test_parse_invalid_digest() {
        assert!(Digest::parse("invalid").is_err());
        assert!(Digest::parse("").is_err());
    }

    #[test]
    fn test_to_blob_path() {
        let digest = Digest::parse("sha256:abc123").unwrap();
        let path = digest.to_blob_path(Path::new("/cache"));
        assert_eq!(path, PathBuf::from("/cache/blobs/sha256/abc123"));
    }
}

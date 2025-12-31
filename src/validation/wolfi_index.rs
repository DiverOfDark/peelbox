//! Wolfi package index for dynamic package version discovery.
//!
//! Two-tier caching: raw tar.gz (24h TTL) + parsed binary cache (30x faster).

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const APKINDEX_URL: &str = "https://packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz";
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
pub struct WolfiPackageIndex {
    packages: HashSet<String>,
}

impl WolfiPackageIndex {
    /// Fetch and parse APKINDEX with two-tier caching (binary cache → tar.gz cache → download).
    pub fn fetch() -> Result<Self> {
        let tar_gz_cache = Self::cache_path()?;
        let parsed_cache = Self::parsed_cache_path()?;

        if let Ok(index) = Self::load_parsed_cache(&parsed_cache, &tar_gz_cache) {
            return Ok(index);
        }

        let content = Self::get_tar_gz_content(&tar_gz_cache)?;
        let index = Self::parse_apkindex(&content)?;
        Self::save_parsed_cache(&parsed_cache, &index)?;

        Ok(index)
    }

    fn get_tar_gz_content(tar_gz_cache: &std::path::Path) -> Result<Vec<u8>> {
        if Self::is_cache_fresh(tar_gz_cache)? {
            return fs::read(tar_gz_cache).with_context(|| {
                format!(
                    "Failed to read cached APKINDEX from {}",
                    tar_gz_cache.display()
                )
            });
        }

        Self::fetch_and_save_tar_gz(tar_gz_cache)
    }

    fn is_cache_fresh(cache_path: &std::path::Path) -> Result<bool> {
        if !cache_path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(cache_path).context("Failed to read cache metadata")?;
        let modified = metadata
            .modified()
            .context("Failed to get cache modification time")?;
        let elapsed = SystemTime::now()
            .duration_since(modified)
            .context("System time is before cache modification time")?;

        Ok(elapsed < CACHE_TTL)
    }

    fn fetch_and_save_tar_gz(cache_path: &std::path::Path) -> Result<Vec<u8>> {
        let content = Self::fetch_apkindex()?;

        if content.is_empty() {
            anyhow::bail!("Downloaded APKINDEX is empty (0 bytes)");
        }

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create cache directory: {}", parent.display())
            })?;
        }

        fs::write(cache_path, &content).with_context(|| {
            format!("Failed to write APKINDEX cache to {}", cache_path.display())
        })?;

        Ok(content)
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = fs::read(path)
            .with_context(|| format!("Failed to read APKINDEX from {}", path.display()))?;

        if content.is_empty() {
            anyhow::bail!("APKINDEX file is empty: {}", path.display());
        }

        Self::parse_apkindex(&content)
    }

    fn fetch_apkindex() -> Result<Vec<u8>> {
        let response = reqwest::blocking::get(APKINDEX_URL).with_context(|| {
            format!(
                "Failed to download APKINDEX from {} (check network connectivity)",
                APKINDEX_URL
            )
        })?;

        if !response.status().is_success() {
            anyhow::bail!(
                "APKINDEX download failed with HTTP {} from {}",
                response.status(),
                APKINDEX_URL
            );
        }

        let bytes = response
            .bytes()
            .context("Failed to read APKINDEX response body")?;

        if bytes.is_empty() {
            anyhow::bail!("Downloaded APKINDEX is empty (HTTP 200 but 0 bytes)");
        }

        Ok(bytes.to_vec())
    }

    fn parse_apkindex(data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            anyhow::bail!("Cannot parse empty APKINDEX data");
        }

        let mut decoder = flate2::read::MultiGzDecoder::new(data);
        let mut tar_data = Vec::new();
        decoder
            .read_to_end(&mut tar_data)
            .context("Failed to decompress APKINDEX.tar.gz (invalid gzip format)")?;

        if tar_data.is_empty() {
            anyhow::bail!("APKINDEX.tar.gz decompressed to empty data");
        }

        let mut archive = tar::Archive::new(&tar_data[..]);
        let mut packages = HashSet::new();

        for entry in archive
            .entries()
            .context("Failed to read tar entries (invalid tar format)")?
        {
            let mut entry = entry.context("Failed to read tar entry")?;
            let path = entry.path().context("Failed to get entry path")?;

            if path.to_str().unwrap_or("") == "APKINDEX" {
                let mut content = Vec::new();
                entry
                    .read_to_end(&mut content)
                    .context("Failed to read APKINDEX content from tar")?;

                let content_str = std::str::from_utf8(&content)
                    .context("APKINDEX contains invalid UTF-8 (expected ASCII text)")?;

                packages.reserve(content_str.len() / 200);

                for line in content_str.lines() {
                    if let Some(package_name) = line.strip_prefix("P:") {
                        let trimmed = package_name.trim();
                        if !trimmed.is_empty() {
                            packages.insert(trimmed.to_string());
                        }
                    }
                }

                break;
            }
        }

        if packages.is_empty() {
            anyhow::bail!(
                "No packages found in APKINDEX (file may be empty or malformed). \
                Expected format: 'P:package-name' lines."
            );
        }

        Ok(Self { packages })
    }

    fn cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .context("Failed to get user cache directory")?
            .join("peelbox")
            .join("apkindex");

        Ok(cache_dir.join("APKINDEX.tar.gz"))
    }

    fn parsed_cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .context("Failed to get user cache directory")?
            .join("peelbox")
            .join("apkindex");

        Ok(cache_dir.join("packages.bin"))
    }

    fn load_parsed_cache(
        parsed_path: &std::path::Path,
        tar_gz_path: &std::path::Path,
    ) -> Result<Self> {
        if !parsed_path.exists() {
            anyhow::bail!("Parsed cache does not exist");
        }

        // Check if tar.gz is newer (indicating cache is stale)
        if tar_gz_path.exists() {
            let parsed_modified = fs::metadata(parsed_path)?.modified()?;
            let tar_gz_modified = fs::metadata(tar_gz_path)?.modified()?;

            if tar_gz_modified > parsed_modified {
                anyhow::bail!("Parsed cache is stale");
            }
        }

        // Load binary cache using bincode
        let data = fs::read(parsed_path).context("Failed to read parsed cache")?;
        let packages: HashSet<String> =
            bincode::deserialize(&data).context("Failed to deserialize parsed cache")?;

        Ok(Self { packages })
    }

    fn save_parsed_cache(parsed_path: &std::path::Path, index: &Self) -> Result<()> {
        if let Some(parent) = parsed_path.parent() {
            fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        let data = bincode::serialize(&index.packages).context("Failed to serialize packages")?;

        fs::write(parsed_path, data).context("Failed to write parsed cache")?;

        Ok(())
    }

    /// Get versions for package prefix, sorted descending with semantic versioning.
    /// Filters out non-numeric versions and package variants.
    pub fn get_versions(&self, package_prefix: &str) -> Vec<String> {
        let mut versions = Vec::new();
        let prefix_with_dash = format!("{}-", package_prefix);

        for package in &self.packages {
            if let Some(version) = package.strip_prefix(&prefix_with_dash) {
                if !version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    continue;
                }

                if version.contains('-') {
                    continue;
                }

                versions.push(version.to_string());
            }
        }

        versions.sort_by(|a, b| {
            let parse_version = |v: &str| -> Vec<u32> {
                v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect()
            };

            let a_parts = parse_version(a);
            let b_parts = parse_version(b);

            for i in 0..a_parts.len().max(b_parts.len()) {
                let a_part = a_parts.get(i).copied().unwrap_or(0);
                let b_part = b_parts.get(i).copied().unwrap_or(0);

                match b_part.cmp(&a_part) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }

            std::cmp::Ordering::Equal
        });

        versions
    }

    /// Get latest version for package prefix (e.g., "nodejs" → "nodejs-22").
    pub fn get_latest_version(&self, package_prefix: &str) -> Option<String> {
        self.get_versions(package_prefix)
            .first()
            .map(|version| format!("{}-{}", package_prefix, version))
    }

    /// Check if exact package name exists (e.g., "build-base", "nodejs-22").
    pub fn has_package(&self, package_name: &str) -> bool {
        self.packages.contains(package_name)
    }

    /// Find best version match (exact or prefix match).
    pub fn match_version(
        &self,
        package_prefix: &str,
        requested: &str,
        available: &[String],
    ) -> Option<String> {
        if available.contains(&requested.to_string()) {
            return Some(format!("{}-{}", package_prefix, requested));
        }

        for version in available {
            if version.starts_with(requested) {
                return Some(format!("{}-{}", package_prefix, version));
            }
        }

        None
    }

    pub fn all_packages(&self) -> Vec<String> {
        let mut packages: Vec<_> = self.packages.iter().cloned().collect();
        packages.sort();
        packages
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        use std::sync::{Arc, OnceLock};

        static TEST_INDEX: OnceLock<Arc<WolfiPackageIndex>> = OnceLock::new();

        let index =
            TEST_INDEX.get_or_init(|| {
                let test_data_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/data/APKINDEX.tar.gz");
                Arc::new(WolfiPackageIndex::from_file(&test_data_path).expect(
                    "Failed to load test APKINDEX - run 'cp /tmp/APKINDEX.tar.gz tests/data/'",
                ))
            });

        (**index).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_versions_nodejs() {
        let index = WolfiPackageIndex::for_tests();
        let versions = index.get_versions("nodejs");
        assert!(!versions.is_empty(), "Should have nodejs versions");
        assert!(versions.contains(&"22".to_string()) || versions.contains(&"23".to_string()));
    }

    #[test]
    fn test_get_versions_python() {
        let index = WolfiPackageIndex::for_tests();
        let versions = index.get_versions("python");
        assert!(!versions.is_empty(), "Should have python versions");
    }

    #[test]
    fn test_get_latest_version() {
        let index = WolfiPackageIndex::for_tests();
        assert!(index.get_latest_version("nodejs").is_some());
        assert!(index.get_latest_version("python").is_some());
        assert!(index.get_latest_version("openjdk").is_some());
    }

    #[test]
    fn test_has_package() {
        let index = WolfiPackageIndex::for_tests();
        // Real Wolfi uses versioned packages
        assert!(index.get_latest_version("rust").is_some());
        assert!(index.has_package("build-base"));
        assert!(index.has_package("ca-certificates"));
        assert!(!index.has_package("nonexistent-package-12345"));
    }

    #[test]
    fn test_match_version_exact() {
        let index = WolfiPackageIndex::for_tests();
        let available = index.get_versions("nodejs");

        if let Some(first_version) = available.first() {
            assert_eq!(
                index.match_version("nodejs", first_version, &available),
                Some(format!("nodejs-{}", first_version))
            );
        }
    }

    #[test]
    fn test_match_version_no_match() {
        let index = WolfiPackageIndex::for_tests();
        let available = vec!["22".to_string(), "20".to_string()];

        assert_eq!(index.match_version("nodejs", "99999", &available), None);
    }
}

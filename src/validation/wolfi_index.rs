use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const APKINDEX_URL: &str = "https://packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz";
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

#[derive(Clone)]
pub struct WolfiPackageIndex {
    packages: HashSet<String>,
}

impl WolfiPackageIndex {
    /// Fetch and parse APKINDEX with 24-hour cache
    /// Also caches the parsed packages list to avoid re-parsing
    pub fn fetch() -> Result<Self> {
        let tar_gz_cache = Self::cache_path()?;
        let parsed_cache = Self::parsed_cache_path()?;

        // Try to load from parsed cache first (much faster)
        if let Ok(index) = Self::load_parsed_cache(&parsed_cache, &tar_gz_cache) {
            return Ok(index);
        }

        // Check if tar.gz cache is valid
        let content = if tar_gz_cache.exists() {
            if let Ok(metadata) = fs::metadata(&tar_gz_cache) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                        if elapsed < CACHE_TTL {
                            // Use cached tar.gz
                            fs::read(&tar_gz_cache)
                                .context("Failed to read cached APKINDEX")?
                        } else {
                            // Expired, fetch new
                            Self::fetch_and_save_tar_gz(&tar_gz_cache)?
                        }
                    } else {
                        Self::fetch_and_save_tar_gz(&tar_gz_cache)?
                    }
                } else {
                    Self::fetch_and_save_tar_gz(&tar_gz_cache)?
                }
            } else {
                Self::fetch_and_save_tar_gz(&tar_gz_cache)?
            }
        } else {
            Self::fetch_and_save_tar_gz(&tar_gz_cache)?
        };

        // Parse and save to parsed cache
        let index = Self::parse_apkindex(&content)?;
        Self::save_parsed_cache(&parsed_cache, &index)?;

        Ok(index)
    }

    /// Fetch APKINDEX and save to cache
    fn fetch_and_save_tar_gz(cache_path: &std::path::Path) -> Result<Vec<u8>> {
        let content = Self::fetch_apkindex()?;

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }
        fs::write(cache_path, &content).context("Failed to write cache file")?;

        Ok(content)
    }

    /// Load APKINDEX from a file (for testing with committed snapshot)
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = fs::read(path)
            .context("Failed to read APKINDEX file")?;
        Self::parse_apkindex(&content)
    }

    /// Download APKINDEX.tar.gz from Wolfi repository (blocking)
    fn fetch_apkindex() -> Result<Vec<u8>> {
        let response = reqwest::blocking::get(APKINDEX_URL)
            .context("Failed to download APKINDEX")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download APKINDEX: HTTP {}", response.status());
        }

        let bytes = response.bytes()
            .context("Failed to read APKINDEX response")?;

        Ok(bytes.to_vec())
    }

    /// Parse APKINDEX.tar.gz file
    fn parse_apkindex(data: &[u8]) -> Result<Self> {
        // Decompress gzip (use MultiGzDecoder to handle multi-member gzip streams)
        let mut decoder = flate2::read::MultiGzDecoder::new(data);
        let mut tar_data = Vec::new();
        decoder.read_to_end(&mut tar_data)
            .context("Failed to decompress APKINDEX.tar.gz")?;

        // Extract tar archive
        let mut archive = tar::Archive::new(&tar_data[..]);
        let mut packages = HashSet::new();

        for entry in archive.entries().context("Failed to read tar entries")? {
            let mut entry = entry.context("Failed to read tar entry")?;
            let path = entry.path().context("Failed to get entry path")?;

            // Look for APKINDEX file (typically just "APKINDEX")
            if path.to_str().unwrap_or("") == "APKINDEX" {
                let mut content = Vec::new();
                entry.read_to_end(&mut content)
                    .context("Failed to read APKINDEX content")?;

                // Parse APK index format efficiently using byte slices
                // Format: Each package separated by blank line, fields start with single letter + colon
                // P:package-name
                // V:version
                // ...
                let content_str = std::str::from_utf8(&content)
                    .context("APKINDEX contains invalid UTF-8")?;

                // Pre-allocate HashSet with estimated capacity
                packages.reserve(content_str.len() / 200); // Rough estimate: ~200 bytes per package entry

                for line in content_str.lines() {
                    if let Some(package_name) = line.strip_prefix("P:") {
                        packages.insert(package_name.trim().to_string());
                    }
                }

                break;
            }
        }

        if packages.is_empty() {
            anyhow::bail!("No packages found in APKINDEX");
        }

        Ok(Self { packages })
    }

    /// Get cache file path (~/.cache/aipack/apkindex/APKINDEX.tar.gz)
    fn cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .context("Failed to get user cache directory")?
            .join("aipack")
            .join("apkindex");

        Ok(cache_dir.join("APKINDEX.tar.gz"))
    }

    /// Get parsed cache file path (~/.cache/aipack/apkindex/packages.bin)
    fn parsed_cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .context("Failed to get user cache directory")?
            .join("aipack")
            .join("apkindex");

        Ok(cache_dir.join("packages.bin"))
    }

    /// Load parsed cache if it exists and is newer than tar.gz
    fn load_parsed_cache(parsed_path: &std::path::Path, tar_gz_path: &std::path::Path) -> Result<Self> {
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
        let packages: HashSet<String> = bincode::deserialize(&data)
            .context("Failed to deserialize parsed cache")?;

        Ok(Self { packages })
    }

    /// Save parsed packages to binary cache
    fn save_parsed_cache(parsed_path: &std::path::Path, index: &Self) -> Result<()> {
        if let Some(parent) = parsed_path.parent() {
            fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        let data = bincode::serialize(&index.packages)
            .context("Failed to serialize packages")?;

        fs::write(parsed_path, data).context("Failed to write parsed cache")?;

        Ok(())
    }

    /// Get all available versions for a package prefix
    /// Example: get_versions("nodejs") -> ["22", "20", "18"]
    /// Filters out non-numeric versions (e.g., "stage0", "doc")
    pub fn get_versions(&self, package_prefix: &str) -> Vec<String> {
        let mut versions = Vec::new();
        let prefix_with_dash = format!("{}-", package_prefix);

        for package in &self.packages {
            if package.starts_with(&prefix_with_dash) {
                // Extract version from package name (e.g., "nodejs-22" -> "22")
                if let Some(version) = package.strip_prefix(&prefix_with_dash) {
                    // Filter out non-numeric versions (stage0, doc, dev, etc.)
                    // Only include versions that start with a digit
                    if !version.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                        continue;
                    }

                    // Filter out package variants (e.g., "openjdk-9-jre-base" -> skip)
                    // Only include base version packages (e.g., "openjdk-9" -> "9")
                    // Version should be: digits optionally followed by dots and more digits
                    // but NOT followed by hyphens (which indicate variants like -jre, -doc, etc.)
                    if version.contains('-') {
                        continue;
                    }

                    versions.push(version.to_string());
                }
            }
        }

        // Sort versions in descending order (highest first)
        // Use numeric comparison for version numbers to handle "21" > "9" and "1.92" > "1.75" correctly
        versions.sort_by(|a, b| {
            // Parse version components (e.g., "1.92" -> [1, 92])
            let parse_version = |v: &str| -> Vec<u32> {
                v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect()
            };

            let a_parts = parse_version(a);
            let b_parts = parse_version(b);

            // Compare component by component (major, minor, patch, etc.)
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

    /// Get latest (highest) version for a package prefix
    /// Returns full package name (e.g., "nodejs-22")
    pub fn get_latest_version(&self, package_prefix: &str) -> Option<String> {
        self.get_versions(package_prefix)
            .first()
            .map(|version| format!("{}-{}", package_prefix, version))
    }

    /// Check if exact package name exists in index
    pub fn has_package(&self, package_name: &str) -> bool {
        self.packages.contains(package_name)
    }

    /// Find best version match for requested version
    /// Example: match_version("nodejs", "18", &["22", "20", "18"]) -> Some("nodejs-18")
    pub fn match_version(
        &self,
        package_prefix: &str,
        requested: &str,
        available: &[String],
    ) -> Option<String> {
        // Try exact match first
        if available.contains(&requested.to_string()) {
            return Some(format!("{}-{}", package_prefix, requested));
        }

        // Try prefix match (e.g., "3.11" matches "3.11.5")
        for version in available {
            if version.starts_with(requested) {
                return Some(format!("{}-{}", package_prefix, version));
            }
        }

        None
    }

    /// Get all package names (for testing and debugging)
    pub fn all_packages(&self) -> Vec<String> {
        let mut packages: Vec<_> = self.packages.iter().cloned().collect();
        packages.sort();
        packages
    }

    /// Load test APKINDEX for unit tests (from committed snapshot)
    /// Uses a static cache to avoid re-parsing in tests
    #[cfg(test)]
    pub fn for_tests() -> Self {
        use std::sync::{Arc, OnceLock};

        static TEST_INDEX: OnceLock<Arc<WolfiPackageIndex>> = OnceLock::new();

        let index = TEST_INDEX.get_or_init(|| {
            let test_data_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data/APKINDEX.tar.gz");
            Arc::new(WolfiPackageIndex::from_file(&test_data_path)
                .expect("Failed to load test APKINDEX - run 'cp /tmp/APKINDEX.tar.gz tests/data/'"))
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

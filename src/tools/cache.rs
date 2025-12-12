//! Tool result caching
//!
//! Caches tool execution results to avoid redundant operations.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cache key combining tool name and arguments
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    tool_name: String,
    arguments: String,
}

impl CacheKey {
    fn new(tool_name: &str, arguments: &Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            arguments: arguments.to_string(),
        }
    }
}

/// Thread-safe cache for tool execution results
#[derive(Clone)]
pub struct ToolCache {
    cache: Arc<RwLock<HashMap<CacheKey, String>>>,
}

impl ToolCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a cached result if available
    pub fn get(&self, tool_name: &str, arguments: &Value) -> Option<String> {
        let key = CacheKey::new(tool_name, arguments);
        self.cache.read().ok()?.get(&key).cloned()
    }

    /// Store a result in the cache
    pub fn insert(&self, tool_name: &str, arguments: &Value, result: String) {
        let key = CacheKey::new(tool_name, arguments);
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(key, result);
        }
    }

    /// Clear all cached results
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get number of cached entries
    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ToolCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_basic_operations() {
        let cache = ToolCache::new();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        let args = json!({"path": "src"});
        cache.insert("list_files", &args, "file1.rs\nfile2.rs".to_string());

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);

        let result = cache.get("list_files", &args);
        assert_eq!(result, Some("file1.rs\nfile2.rs".to_string()));
    }

    #[test]
    fn test_cache_miss() {
        let cache = ToolCache::new();

        let args1 = json!({"path": "src"});
        let args2 = json!({"path": "tests"});

        cache.insert("list_files", &args1, "file1.rs".to_string());

        assert_eq!(
            cache.get("list_files", &args1),
            Some("file1.rs".to_string())
        );
        assert_eq!(cache.get("list_files", &args2), None);
        assert_eq!(cache.get("read_file", &args1), None);
    }

    #[test]
    fn test_cache_clear() {
        let cache = ToolCache::new();

        cache.insert(
            "list_files",
            &json!({"path": "src"}),
            "file1.rs".to_string(),
        );
        cache.insert(
            "read_file",
            &json!({"path": "README.md"}),
            "content".to_string(),
        );

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_different_arguments() {
        let cache = ToolCache::new();

        cache.insert("list_files", &json!({"path": "src"}), "result1".to_string());
        cache.insert(
            "list_files",
            &json!({"path": "src", "pattern": "*.rs"}),
            "result2".to_string(),
        );

        assert_eq!(cache.len(), 2);

        assert_eq!(
            cache.get("list_files", &json!({"path": "src"})),
            Some("result1".to_string())
        );
        assert_eq!(
            cache.get("list_files", &json!({"path": "src", "pattern": "*.rs"})),
            Some("result2".to_string())
        );
    }

    #[test]
    fn test_cache_thread_safety() {
        use std::thread;

        let cache = ToolCache::new();
        let cache_clone = cache.clone();

        let handle = thread::spawn(move || {
            cache_clone.insert("list_files", &json!({"path": "src"}), "result".to_string());
        });

        handle.join().unwrap();

        assert_eq!(
            cache.get("list_files", &json!({"path": "src"})),
            Some("result".to_string())
        );
    }
}

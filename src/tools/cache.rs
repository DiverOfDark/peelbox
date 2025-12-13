use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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

#[derive(Clone)]
pub struct ToolCache {
    cache: Arc<RwLock<HashMap<CacheKey, Value>>>,
}

impl ToolCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, tool_name: &str, arguments: &Value) -> Option<Value> {
        let key = CacheKey::new(tool_name, arguments);
        self.cache.read().ok()?.get(&key).cloned()
    }

    pub fn insert(&self, tool_name: &str, arguments: &Value, result: Value) {
        let key = CacheKey::new(tool_name, arguments);
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(key, result);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

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
        cache.insert("list_files", &args, json!("file1.rs\nfile2.rs"));

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);

        let result = cache.get("list_files", &args);
        assert_eq!(result, Some(json!("file1.rs\nfile2.rs")));
    }

    #[test]
    fn test_cache_miss() {
        let cache = ToolCache::new();

        let args1 = json!({"path": "src"});
        let args2 = json!({"path": "tests"});

        cache.insert("list_files", &args1, json!("file1.rs"));

        assert_eq!(
            cache.get("list_files", &args1),
            Some(json!("file1.rs"))
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
            json!("file1.rs"),
        );
        cache.insert(
            "read_file",
            &json!({"path": "README.md"}),
            json!("content"),
        );

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_different_arguments() {
        let cache = ToolCache::new();

        cache.insert("list_files", &json!({"path": "src"}), json!("result1"));
        cache.insert(
            "list_files",
            &json!({"path": "src", "pattern": "*.rs"}),
            json!("result2"),
        );

        assert_eq!(cache.len(), 2);

        assert_eq!(
            cache.get("list_files", &json!({"path": "src"})),
            Some(json!("result1"))
        );
        assert_eq!(
            cache.get("list_files", &json!({"path": "src", "pattern": "*.rs"})),
            Some(json!("result2"))
        );
    }

    #[test]
    fn test_cache_thread_safety() {
        use std::thread;

        let cache = ToolCache::new();
        let cache_clone = cache.clone();

        let handle = thread::spawn(move || {
            cache_clone.insert("list_files", &json!({"path": "src"}), json!("result"));
        });

        handle.join().unwrap();

        assert_eq!(
            cache.get("list_files", &json!({"path": "src"})),
            Some(json!("result"))
        );
    }
}

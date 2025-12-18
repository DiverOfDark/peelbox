//! Common directory scanning utilities for extractors

use crate::fs::FileSystem;
use crate::languages::LanguageDefinition;
use std::path::Path;

/// Scans a directory for files matching language-specific criteria and applies a callback
///
/// This function provides a reusable pattern for directory traversal that:
/// 1. Walks the directory entries
/// 2. Filters for regular files only
/// 3. Checks if files match language-specific criteria (e.g., is_main_file)
/// 4. Applies the provided callback to matching files
///
/// # Arguments
/// * `fs` - FileSystem implementation for directory reading
/// * `dir_path` - Path to the directory to scan
/// * `lang` - Language definition for file matching
/// * `callback` - Function to apply to each matching file path
pub fn scan_directory_with_language_filter<F, C>(
    fs: &F,
    dir_path: &Path,
    lang: &dyn LanguageDefinition,
    mut callback: C,
) where
    F: FileSystem,
    C: FnMut(&Path),
{
    let entries = match fs.read_dir(dir_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        if entry.file_type() != crate::fs::FileType::File {
            continue;
        }

        let file_path = entry.path();
        if !lang.is_main_file(fs, file_path) {
            continue;
        }

        callback(file_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::MockFileSystem;
    use crate::stack::registry::StackRegistry;
    use std::path::PathBuf;

    #[test]
    fn test_scan_directory_filters_files() {
        let fs = MockFileSystem::new();
        fs.add_file("server.js", "content");
        fs.add_file("README.md", "docs");
        fs.add_file("package.json", "{}");

        let registry = StackRegistry::with_defaults();
        let lang = registry
            .get_language(crate::stack::LanguageId::JavaScript)
            .unwrap();

        let mut found_files = Vec::new();
        scan_directory_with_language_filter(&fs, Path::new("."), lang, |path| {
            found_files.push(path.to_path_buf());
        });

        assert_eq!(found_files.len(), 1);
        assert!(found_files[0].ends_with("server.js"));
    }

    #[test]
    fn test_scan_directory_handles_empty_dir() {
        let fs = MockFileSystem::new();
        let registry = StackRegistry::with_defaults();
        let lang = registry
            .get_language(crate::stack::LanguageId::JavaScript)
            .unwrap();

        let mut found_files: Vec<PathBuf> = Vec::new();
        scan_directory_with_language_filter(&fs, Path::new("."), lang, |path| {
            found_files.push(path.to_path_buf());
        });

        assert_eq!(found_files.len(), 0);
    }

    #[test]
    fn test_scan_directory_handles_nonexistent_dir() {
        let fs = MockFileSystem::new();
        let registry = StackRegistry::with_defaults();
        let lang = registry
            .get_language(crate::stack::LanguageId::JavaScript)
            .unwrap();

        let mut found_files: Vec<PathBuf> = Vec::new();
        scan_directory_with_language_filter(&fs, Path::new("nonexistent"), lang, |path| {
            found_files.push(path.to_path_buf());
        });

        assert_eq!(found_files.len(), 0);
    }
}

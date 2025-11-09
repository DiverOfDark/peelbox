//! Manifest file patterns and exclusion rules

use std::path::Path;

/// Common manifest files that indicate build systems
pub const MANIFEST_PATTERNS: &[&str] = &[
    // Rust
    "Cargo.toml",
    // JavaScript/TypeScript
    "package.json",
    // Java
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "gradlew",
    "build.sbt",
    // Python
    "requirements.txt",
    "Pipfile",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    // Ruby
    "Gemfile",
    // PHP
    "composer.json",
    // Go
    "go.mod",
    // Elixir
    "mix.exs",
    // .NET
    "*.csproj",
    "*.fsproj",
    "*.vbproj",
    "*.sln",
    // C/C++
    "Makefile",
    "CMakeLists.txt",
    "meson.build",
    // Workspace configurations
    "pnpm-workspace.yaml",
    "lerna.json",
    "nx.json",
    "turbo.json",
    "rush.json",
    // Docker
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    ".dockerignore",
    // Node version
    ".nvmrc",
    ".node-version",
];

/// Directories to exclude from scanning
pub const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".output",
    "venv",
    ".venv",
    "env",
    ".env",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    "vendor",
    ".idea",
    ".vscode",
    ".vs",
    "coverage",
    ".coverage",
    "htmlcov",
    ".tox",
    ".eggs",
    "*.egg-info",
    ".gradle",
    ".m2",
    ".cargo",
];

/// File patterns to exclude
pub const EXCLUDED_FILES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];

/// Maximum scan depth
pub const MAX_SCAN_DEPTH: usize = 10;

/// Maximum number of files to scan
pub const MAX_FILES: usize = 1000;

/// Checks if a directory should be excluded from scanning
pub fn is_excluded_dir(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        // Check exact matches
        if EXCLUDED_DIRS.contains(&name) {
            return true;
        }
        // Check hidden directories
        if name.starts_with('.') && name.len() > 1 {
            return true;
        }
        // Check temporary and log files
        if name.ends_with(".tmp") || name.ends_with(".log") {
            return true;
        }
    }
    false
}

/// Checks if a file should be excluded from scanning
pub fn is_excluded_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if EXCLUDED_FILES.contains(&name) {
            return true;
        }
        // Exclude temporary and backup files
        if name.ends_with('~')
            || name.ends_with(".bak")
            || name.ends_with(".swp")
            || name.starts_with(".#")
        {
            return true;
        }
    }
    false
}

/// Checks if a filename matches any manifest pattern
pub fn is_manifest_file(filename: &str) -> bool {
    MANIFEST_PATTERNS.iter().any(|pattern| {
        if pattern.contains('*') {
            // Simple glob matching for *.ext patterns
            if let Some(ext) = pattern.strip_prefix("*.") {
                filename.ends_with(&format!(".{}", ext))
            } else {
                false
            }
        } else {
            // Exact match
            filename == *pattern
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_excluded_dir() {
        assert!(is_excluded_dir(&PathBuf::from("node_modules")));
        assert!(is_excluded_dir(&PathBuf::from("target")));
        assert!(is_excluded_dir(&PathBuf::from(".git")));
        assert!(is_excluded_dir(&PathBuf::from(".hidden")));
        assert!(!is_excluded_dir(&PathBuf::from("src")));
        assert!(!is_excluded_dir(&PathBuf::from("lib")));
    }

    #[test]
    fn test_is_excluded_file() {
        assert!(is_excluded_file(&PathBuf::from(".DS_Store")));
        assert!(is_excluded_file(&PathBuf::from("file.bak")));
        assert!(is_excluded_file(&PathBuf::from("file~")));
        assert!(is_excluded_file(&PathBuf::from(".#file")));
        assert!(!is_excluded_file(&PathBuf::from("Cargo.toml")));
        assert!(!is_excluded_file(&PathBuf::from("package.json")));
    }

    #[test]
    fn test_is_manifest_file() {
        // Exact matches
        assert!(is_manifest_file("Cargo.toml"));
        assert!(is_manifest_file("package.json"));
        assert!(is_manifest_file("pom.xml"));
        assert!(is_manifest_file("go.mod"));
        assert!(is_manifest_file("Gemfile"));

        // Glob patterns
        assert!(is_manifest_file("project.csproj"));
        assert!(is_manifest_file("app.fsproj"));
        assert!(is_manifest_file("solution.sln"));

        // Non-matches
        assert!(!is_manifest_file("README.md"));
        assert!(!is_manifest_file("main.rs"));
        assert!(!is_manifest_file("index.js"));
    }
}

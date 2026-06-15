use super::*;

/// Scan for language version files (.nvmrc, .node-version, .python-version)
/// and update package names to include specific versions.
pub(crate) fn scan_version_files(repo_root: &Path, build: &mut UniversalBuild) {
    let language = &build.metadata.language;
    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    match language.as_str() {
        "JavaScript" | "TypeScript" => {
            if let Some(version) = read_node_version(&project_dir, repo_root) {
                let versioned_pkg = format!("nodejs-{}", version);
                replace_package(&mut build.build.packages, "nodejs", &versioned_pkg);
                replace_package(&mut build.runtime.packages, "nodejs", &versioned_pkg);
            }
        }
        "Python" => {
            if let Some(version) = read_python_version(&project_dir, repo_root) {
                let versioned_pkg = format!("python-{}", version);
                replace_package(&mut build.build.packages, "python", &versioned_pkg);
                replace_package(&mut build.runtime.packages, "python", &versioned_pkg);
            }
        }
        "PHP" => {
            if let Some(version) = read_php_version(&project_dir, repo_root) {
                let versioned_pkg = format!("php-{}", version);
                replace_package(&mut build.build.packages, "php", &versioned_pkg);
                replace_package(&mut build.runtime.packages, "php", &versioned_pkg);
            }
        }
        "Ruby" => {
            if let Some(version) = read_ruby_version(&project_dir, repo_root) {
                let versioned_pkg = format!("ruby-{}", version);
                let versioned_dev = format!("ruby-{}-dev", version);
                replace_package(&mut build.build.packages, "ruby", &versioned_pkg);
                replace_package(&mut build.build.packages, "ruby-dev", &versioned_dev);
                replace_package(&mut build.runtime.packages, "ruby", &versioned_pkg);
            }
        }
        "Rust" => {
            // Only build packages need the rust compiler; runtime uses the compiled binary
            if let Some(version) = read_rust_version(&project_dir, repo_root) {
                let versioned_pkg = format!("rust-{}", version);
                replace_package(&mut build.build.packages, "rust", &versioned_pkg);
            }
        }
        _ => {}
    }
}

use std::path::{Path, PathBuf};

/// Check if a directory contains any files (recursively).
pub fn has_any_file(path: &Path) -> bool {
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_file() {
            return true;
        }
        if ft.is_dir() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            if has_any_file(&entry.path()) {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Fixture {
    pub name: String,
    pub category: String,
    pub path: PathBuf,
    pub has_snapshot: bool,
    /// When true, the test is registered as ignored (e.g. empty compat snapshot).
    pub ignore: bool,
}

#[allow(dead_code)]
pub fn find_fixtures() -> Vec<Fixture> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let base_path = Path::new(&manifest_dir).join("tests/fixtures");

    let categories = vec!["single-language", "monorepo", "edge-cases"];
    let mut fixtures = Vec::new();

    for category in categories {
        let category_path = base_path.join(category);
        if !category_path.exists() {
            continue;
        }

        let entries = std::fs::read_dir(&category_path)
            .unwrap_or_else(|_| panic!("Failed to read directory: {:?}", category_path));

        for entry in entries {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();

                if name.starts_with('.') {
                    continue;
                }

                // Skip empty directories (no files anywhere)
                let has_files = has_any_file(&path);
                if !has_files {
                    continue;
                }

                let has_snapshot = path.join("universalbuild.json").exists();

                fixtures.push(Fixture {
                    name,
                    category: category.to_string(),
                    path,
                    has_snapshot,
                    ignore: false,
                });
            }
        }
    }

    fixtures.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));

    // Append compat fixtures from external repos (railpack, nixpacks).
    // Returns empty if ./scripts/fetch-compat-fixtures.sh hasn't been run.
    if super::compat_discovery::should_update_snapshots() {
        // In update mode, include ALL external examples (even without snapshots)
        // so we can generate snapshots for them.
        fixtures.extend(super::compat_discovery::find_all_external_examples());
    } else {
        // Normal mode: only include fixtures with committed snapshots.
        fixtures.extend(super::compat_discovery::find_compat_fixtures());
    }

    fixtures
}

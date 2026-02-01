use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Fixture {
    pub name: String,
    pub category: String,
    pub path: PathBuf,
    pub has_snapshot: bool,
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

                let has_snapshot = path.join("universalbuild.json").exists();

                fixtures.push(Fixture {
                    name,
                    category: category.to_string(),
                    path,
                    has_snapshot,
                });
            }
        }
    }

    fixtures.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));

    fixtures
}

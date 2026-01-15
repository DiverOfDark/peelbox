use std::path::Path;

fn normalize_python_version(version_str: &str) -> Option<String> {
    let ver = version_str
        .trim()
        .trim_start_matches(">=")
        .trim_start_matches("^")
        .trim_start_matches("~")
        .trim_start_matches("python")
        .trim()
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    if !ver.is_empty() {
        Some(format!("python-{}", ver))
    } else {
        None
    }
}

pub(super) fn read_python_version_file(service_path: &Path) -> Option<String> {
    let runtime_txt = service_path.join("runtime.txt");
    if let Ok(content) = std::fs::read_to_string(&runtime_txt) {
        if !content.trim().is_empty() {
            return normalize_python_version(&content);
        }
    }

    let python_version = service_path.join(".python-version");
    if let Ok(content) = std::fs::read_to_string(&python_version) {
        if !content.trim().is_empty() {
            return normalize_python_version(&content);
        }
    }

    None
}

pub(super) fn parse_pyproject_toml_version(manifest_content: &str) -> Option<String> {
    for line in manifest_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("requires-python") {
            if let Some(eq_pos) = trimmed.find('=') {
                let value = &trimmed[eq_pos + 1..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                return normalize_python_version(value);
            }
        }
    }
    None
}

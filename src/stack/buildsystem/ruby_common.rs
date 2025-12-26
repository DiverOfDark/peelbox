use std::path::Path;

fn normalize_ruby_version(version_str: &str) -> Option<String> {
    let ver = version_str
        .trim()
        .trim_start_matches("ruby")
        .trim()
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    if !ver.is_empty() {
        Some(format!("ruby-{}", ver))
    } else {
        None
    }
}

pub(super) fn read_ruby_version_file(service_path: &Path) -> Option<String> {
    let ruby_version_file = service_path.join(".ruby-version");
    if let Ok(content) = std::fs::read_to_string(&ruby_version_file) {
        if !content.trim().is_empty() {
            return normalize_ruby_version(&content);
        }
    }
    None
}

pub(super) fn parse_gemfile_version(manifest_content: &str) -> Option<String> {
    for line in manifest_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ruby") && trimmed.contains('"') {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() >= 2 {
                return normalize_ruby_version(parts[1]);
            }
        }
    }
    None
}

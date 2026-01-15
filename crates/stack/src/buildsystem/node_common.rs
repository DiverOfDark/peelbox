use std::path::Path;

pub(super) fn normalize_node_version(version_str: &str) -> Option<String> {
    let ver_num = version_str
        .trim()
        .trim_start_matches("v")
        .trim_start_matches(">=")
        .trim_start_matches("^")
        .trim_start_matches("~")
        .split('.')
        .next()?;
    Some(format!("nodejs-{}", ver_num))
}

pub(super) fn read_node_version_file(service_path: &Path) -> Option<String> {
    for file_name in [".nvmrc", ".node-version"] {
        let path = service_path.join(file_name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if !content.trim().is_empty() {
                return normalize_node_version(&content);
            }
        }
    }
    None
}

pub(super) fn parse_node_version(manifest_content: &str) -> Option<String> {
    let package: serde_json::Value = serde_json::from_str(manifest_content).ok()?;
    let node_version = package["engines"]["node"].as_str()?;
    normalize_node_version(node_version)
}

use super::{Dependency, DependencyInfo, DetectionMethod};
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;

pub trait DependencyParser: Send + Sync {
    fn parse(&self, content: &str, all_internal_paths: &[PathBuf]) -> DependencyInfo;
}

pub struct TomlDependencyParser {
    pub dependencies_keys: &'static [&'static str],
    pub workspace_members_key: Option<&'static str>,
}

impl DependencyParser for TomlDependencyParser {
    fn parse(&self, content: &str, _all_internal_paths: &[PathBuf]) -> DependencyInfo {
        let parsed: toml::Value = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return DependencyInfo::empty(),
        };

        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        for dep_section in self.dependencies_keys {
            if let Some(deps) = parsed.get(dep_section).and_then(|v| v.as_table()) {
                for (name, value) in deps {
                    if seen.contains(name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let (version, is_internal) = if let Some(table) = value.as_table() {
                        let version = table
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let is_path = table.get("path").is_some();
                        (version, is_path)
                    } else if let Some(ver) = value.as_str() {
                        (Some(ver.to_string()), false)
                    } else {
                        (None, false)
                    };

                    let dep = Dependency {
                        name: name.clone(),
                        version,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
                    }
                }
            }
        }

        if let Some(workspace_key) = self.workspace_members_key {
            if let Some(workspace) = parsed.get("workspace").and_then(|v| v.as_table()) {
                if let Some(members) = workspace.get(workspace_key).and_then(|v| v.as_array()) {
                    for member in members {
                        if let Some(member_name) = member.as_str() {
                            let name = member_name
                                .split('/')
                                .next_back()
                                .unwrap_or(member_name)
                                .to_string();
                            if !seen.contains(&name) {
                                internal_deps.push(Dependency {
                                    name: name.clone(),
                                    version: Some("workspace".to_string()),
                                    is_internal: true,
                                });
                                seen.insert(name);
                            }
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }
}

pub struct JsonDependencyParser {
    pub dependencies_keys: &'static [&'static str],
    pub workspace_key: Option<&'static str>,
}

impl DependencyParser for JsonDependencyParser {
    fn parse(&self, content: &str, all_internal_paths: &[PathBuf]) -> DependencyInfo {
        let parsed: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return DependencyInfo::empty(),
        };

        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        for dep_type in self.dependencies_keys {
            if let Some(deps) = parsed.get(dep_type).and_then(|v| v.as_object()) {
                for (name, version) in deps {
                    if seen.contains(name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let version_str = version.as_str().map(|s| s.to_string());

                    let is_internal = if let Some(v) = version_str.as_deref() {
                        v.starts_with("file:")
                            || v.starts_with("workspace:")
                            || v.starts_with("link:")
                    } else {
                        false
                    };

                    let dep = Dependency {
                        name: name.clone(),
                        version: version_str,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
                    }
                }
            }
        }

        if let Some(workspace_key) = self.workspace_key {
            if let Some(workspaces) = parsed.get(workspace_key) {
                let workspace_patterns: Vec<String> = if let Some(arr) = workspaces.as_array() {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                } else if let Some(obj) = workspaces.as_object() {
                    if let Some(packages) = obj.get("packages").and_then(|v| v.as_array()) {
                        packages
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                for pattern in workspace_patterns {
                    for path in all_internal_paths {
                        if let Some(path_str) = path.to_str() {
                            if path_str.contains(&pattern.replace("/*", "")) {
                                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                    if !seen.contains(name) {
                                        internal_deps.push(Dependency {
                                            name: name.to_string(),
                                            version: Some("workspace:*".to_string()),
                                            is_internal: true,
                                        });
                                        seen.insert(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }
}

pub struct RegexDependencyParser {
    pub line_pattern: Regex,
    pub internal_check: fn(&str, &[PathBuf]) -> bool,
}

impl DependencyParser for RegexDependencyParser {
    fn parse(&self, content: &str, all_internal_paths: &[PathBuf]) -> DependencyInfo {
        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(caps) = self.line_pattern.captures(line) {
                if let Some(name_match) = caps.get(1) {
                    let name = name_match.as_str().to_string();
                    if seen.contains(&name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let version = caps.get(2).map(|m| m.as_str().to_string());
                    let is_internal = (self.internal_check)(&name, all_internal_paths);

                    let dep = Dependency {
                        name,
                        version,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }
}

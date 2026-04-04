//! Generic source-file scanning infrastructure.
//!
//! Three scanners — ports, health endpoints, and environment variables — all
//! follow the same pattern: look up language-specific regex tables, walk
//! source files with matching extensions, apply the patterns, and collect
//! results.  Pattern data is registered per-language via `inventory::submit!`
//! in the parser files; this module provides the shared walker and scanners.

use crate::helpers::extract_project_dir;
use ignore::WalkBuilder;
use peelbox_core::output::schema::{HealthCheck, UniversalBuild};
use std::collections::HashSet;
use std::path::Path;
use tracing::debug;

// ── Inventory-based pattern registration ────────────────────────────────

/// Language-specific source scanning patterns, registered via `inventory::submit!`
/// in each parser file.
pub struct SourceScanEntry {
    pub languages: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub port_patterns: &'static [&'static str],
    pub health_patterns: &'static [&'static str],
    pub env_var_patterns: &'static [&'static str],
}

inventory::collect!(SourceScanEntry);

/// Built-in environment variables to skip during env-var scanning.
pub const BUILTIN_ENV_VARS: &[&str] = &["PATH", "HOME", "USER", "SHELL", "LANG", "TERM"];

// ── Generic walker ───────────────────────────────────────────────────────

/// Resolve patterns for a given language from the inventory-registered entries.
fn resolve_patterns(
    language: &str,
    get_patterns: fn(&SourceScanEntry) -> &'static [&'static str],
) -> Option<(&'static [&'static str], Vec<regex::Regex>)> {
    for entry in inventory::iter::<SourceScanEntry> {
        if entry.languages.contains(&language) {
            let patterns = get_patterns(entry);
            if patterns.is_empty() {
                return None;
            }
            let compiled: Vec<regex::Regex> = patterns
                .iter()
                .filter_map(|p| regex::Regex::new(p).ok())
                .collect();
            if compiled.is_empty() {
                return None;
            }
            return Some((entry.extensions, compiled));
        }
    }
    None
}

/// Callback provided to [`walk_source_files`].
type MatchCallback<'a> = &'a mut dyn FnMut(&str, &[regex::Regex], &Path) -> bool;

/// Walk source files under `project_dir` whose extension matches
/// `extensions`, read each file, and invoke `callback`.  The walk stops
/// early if the callback returns `true`.
fn walk_source_files(
    project_dir: &Path,
    extensions: &[&str],
    compiled: &[regex::Regex],
    skip_dev_test: bool,
    callback: MatchCallback<'_>,
) {
    if !project_dir.is_dir() {
        return;
    }

    let walker = WalkBuilder::new(project_dir)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if !extensions.contains(&ext) {
            continue;
        }

        // Skip dev/test configuration files when requested.
        if skip_dev_test {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let stem_lower = stem.to_ascii_lowercase();
                if stem_lower.contains("dev")
                    || stem_lower.contains("test")
                    || stem_lower.contains("development")
                {
                    continue;
                }
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let stop = callback(&content, compiled, path);
        if stop {
            return;
        }
    }
}

// ── Public scanners ──────────────────────────────────────────────────────

/// Scan source files in a project directory for port patterns.
/// When ports are found in source code, they replace the framework default ports.
pub fn scan_source_ports(repo_root: &Path, build: &mut UniversalBuild) {
    let language = &build.metadata.language;

    let (extensions, compiled) = match resolve_patterns(language.as_str(), |e| e.port_patterns) {
        Some(v) => v,
        None => return,
    };

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    let mut source_ports: Vec<u16> = Vec::new();
    let mut seen = HashSet::new();

    walk_source_files(
        &project_dir,
        extensions,
        &compiled,
        true, // skip dev/test files
        &mut |content, regexes, path| {
            for re in regexes {
                for cap in re.captures_iter(content) {
                    if let Some(port_match) = cap.get(1) {
                        if let Ok(port) = port_match.as_str().parse::<u16>() {
                            if port >= 1024 && seen.insert(port) {
                                debug!(port, file = %path.display(), "Found port in source code");
                                source_ports.push(port);
                            }
                        }
                    }
                }
            }
            false // keep walking
        },
    );

    if !source_ports.is_empty() {
        source_ports.sort();
        source_ports.dedup();

        // Sync FLASK_RUN_PORT if source code declares a different port
        if let Some(flask_port) = build.runtime.env.get("FLASK_RUN_PORT").cloned() {
            if let Some(&first_port) = source_ports.first() {
                let flask_port_num = flask_port.parse::<u16>().unwrap_or(0);
                if flask_port_num != first_port {
                    debug!(
                        old_port = flask_port_num,
                        new_port = first_port,
                        "Syncing FLASK_RUN_PORT with source-detected port"
                    );
                    build
                        .runtime
                        .env
                        .insert("FLASK_RUN_PORT".into(), first_port.to_string());
                }
            }
        }

        if build.runtime.ports.is_empty() {
            build.runtime.ports = source_ports;
        } else {
            let mut merged = source_ports;
            for port in &build.runtime.ports {
                if !merged.contains(port) {
                    merged.push(*port);
                }
            }
            build.runtime.ports = merged;
        }
    }
}

/// Scan source files for health endpoint patterns.
/// Only sets health if not already set by config or framework.
pub fn scan_source_health(repo_root: &Path, build: &mut UniversalBuild) {
    if build.runtime.health.is_some() {
        return;
    }

    let language = &build.metadata.language;

    let (extensions, compiled) = match resolve_patterns(language.as_str(), |e| e.health_patterns) {
        Some(v) => v,
        None => return,
    };

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    let mut found_endpoint: Option<String> = None;

    walk_source_files(
        &project_dir,
        extensions,
        &compiled,
        false, // don't skip dev/test — health endpoints are relevant everywhere
        &mut |content, regexes, path| {
            for re in regexes {
                if let Some(cap) = re.captures(content) {
                    if let Some(endpoint) = cap.get(1) {
                        let ep = endpoint.as_str().to_string();
                        debug!(endpoint = %ep, file = %path.display(), "Found health endpoint in source code");
                        found_endpoint = Some(ep);
                        return true; // stop walking
                    }
                }
            }
            false
        },
    );

    if let Some(ep) = found_endpoint {
        build.runtime.health = Some(HealthCheck { endpoint: ep });
    }
}

/// Scan source files for environment variable references.
/// Adds discovered vars to runtime env with empty values (only if not already present).
pub fn scan_source_env_vars(repo_root: &Path, build: &mut UniversalBuild) {
    let language = &build.metadata.language;

    let (extensions, compiled) = match resolve_patterns(language.as_str(), |e| e.env_var_patterns) {
        Some(v) => v,
        None => return,
    };

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    walk_source_files(
        &project_dir,
        extensions,
        &compiled,
        false, // don't skip dev/test — env vars are relevant everywhere
        &mut |content, regexes, path| {
            for re in regexes {
                for cap in re.captures_iter(content) {
                    if let Some(var_match) = cap.get(1) {
                        let var_name = var_match.as_str();
                        if BUILTIN_ENV_VARS.contains(&var_name) {
                            continue;
                        }
                        if !build.runtime.env.contains_key(var_name) {
                            debug!(var = var_name, file = %path.display(), "Found env var in source code");
                            let default_value = if language == "Elixir" {
                                extract_elixir_fallback(content, var_name)
                            } else {
                                String::new()
                            };
                            build
                                .runtime
                                .env
                                .insert(var_name.to_string(), default_value);
                        }
                    }
                }
            }
            false // keep walking
        },
    );
}

/// Extract fallback default value from Elixir `System.get_env("VAR") || "default"` patterns.
fn extract_elixir_fallback(content: &str, var_name: &str) -> String {
    let pattern = format!(
        r#"System\.get_env\(["']{}["']\)\s*\|\|\s*"([^"]*)""#,
        regex::escape(var_name)
    );
    if let Ok(re) = regex::Regex::new(&pattern) {
        if let Some(cap) = re.captures(content) {
            if let Some(m) = cap.get(1) {
                return m.as_str().to_string();
            }
        }
    }
    String::new()
}

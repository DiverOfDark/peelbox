//! Generic source-file scanning infrastructure.
//!
//! Three scanners — ports, health endpoints, and environment variables — all
//! follow the same pattern: look up language-specific regex tables, walk
//! source files with matching extensions, apply the patterns, and collect
//! results.  This module centralises the pattern tables and the shared
//! walker logic so the individual scanners are thin wrappers.

use crate::helpers::extract_project_dir;
use ignore::WalkBuilder;
use peelbox_core::output::schema::{HealthCheck, UniversalBuild};
use std::collections::HashSet;
use std::path::Path;
use tracing::debug;

// ── Pattern tables ───────────────────────────────────────────────────────

/// Each entry: `(languages, file_extensions, regex_patterns)`.
///
/// A scanner finds the first entry whose `languages` slice contains the
/// build's language string, then walks source files whose extension is in
/// `file_extensions`, applying every regex in `regex_patterns`.
pub type PatternTable = &'static [(
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
)];

/// Language-specific regex patterns for detecting **port** numbers in source code.
pub const PORT_PATTERNS: PatternTable = &[
    (
        &["Rust"],
        &["rs"],
        &[
            r#"[.:]+bind\([^,)]*:(\d{4,5})"#,
            r#"[.:]+bind\(\("[^"]*",\s*(\d{4,5})\)"#,
            r#"addr\s*=\s*"[^:]*:(\d{4,5})""#,
        ],
    ),
    (
        &["JavaScript", "TypeScript"],
        &["js", "ts", "mjs", "cjs"],
        &[
            r"\.listen\(\s*(\d{4,5})",
            r#"port["\s:=]+(\d{4,5})"#,
            r"\|\|\s*(\d{4,5})",
        ],
    ),
    (
        &["Python"],
        &["py"],
        &[
            r"\.run\([^)]*port\s*=\s*(\d{4,5})",
            r#"port\s*=\s*(\d{4,5})"#,
        ],
    ),
    (
        &["Go"],
        &["go"],
        &[
            r"ListenAndServe\([^)]*:(\d{4,5})",
            r#"addr\s*=\s*"[^:]*:(\d{4,5})""#,
        ],
    ),
    (
        &["Java", "Kotlin"],
        &["java", "kt", "kts", "properties", "yml", "yaml"],
        &[
            r"\.setPort\(\s*(\d{4,5})\s*\)",
            r#"server\.port\s*=\s*(\d{4,5})"#,
        ],
    ),
    (
        &["Scala"],
        &["scala", "java", "properties", "yml", "yaml"],
        &[
            r"\.setPort\(\s*(\d{4,5})\s*\)",
            r#"server\.port\s*=\s*(\d{4,5})"#,
            r#"port\s*=\s*(\d{4,5})"#,
        ],
    ),
    (&["Elixir"], &["ex", "exs"], &[r#"port:\s*(\d{4,5})"#]),
    (
        &["Ruby"],
        &["rb"],
        &[r#"set\s*:port\s*,\s*(\d{4,5})"#, r#"port\s*=\s*(\d{4,5})"#],
    ),
    (
        &["C#"],
        &["cs"],
        &[
            r#"UseUrls\([^)]*:(\d{4,5})"#,
            r#"app\.Run\([^)]*:(\d{4,5})"#,
            r#"\.UsePort\(\s*(\d{4,5})\s*\)"#,
        ],
    ),
    (
        &["F#"],
        &["fs"],
        &[
            r#"UseUrls\([^)]*:(\d{4,5})"#,
            r#"\.UsePort\(\s*(\d{4,5})\s*\)"#,
        ],
    ),
    (
        &["PHP"],
        &["php"],
        &[r#"'PORT'\s*,\s*(\d{4,5})"#, r#"\$port\s*=\s*(\d{4,5})"#],
    ),
    (
        &["C"],
        &["c", "h"],
        &[r#"htons\(\s*(\d{4,5})\s*\)"#, r#"port\s*=\s*(\d{4,5})"#],
    ),
    (
        &["C++"],
        &["cpp", "cxx", "cc", "hpp", "h"],
        &[r#"htons\(\s*(\d{4,5})\s*\)"#, r#"port\s*=\s*(\d{4,5})"#],
    ),
    (
        &["Clojure"],
        &["clj", "cljc", "cljs"],
        &[
            r#":port\s+(\d{4,5})"#,
            r#"\{:port\s+(\d{4,5})\}"#,
            r#"run-jetty\s+[^\{]*\{[^}]*:port\s+(\d{4,5})"#,
        ],
    ),
];

/// Language-specific regex patterns for detecting **health endpoints** in source code.
pub const HEALTH_PATTERNS: PatternTable = &[
    (
        &["JavaScript", "TypeScript"],
        &["js", "ts", "mjs", "cjs"],
        &[r#"app\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        &["Java", "Kotlin"],
        &["java", "kt", "kts"],
        &[r#"@(?:Get|Request)Mapping\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        &["Scala"],
        &["scala", "java"],
        &[
            r#"path\(['"]([/\w\-]*health[/\w\-]*)['"]"#,
            r#"@(?:Get|Request)Mapping\(['"]([/\w\-]*health[/\w\-]*)['"]"#,
        ],
    ),
    (
        &["Python"],
        &["py"],
        &[r#"@app\.(?:get|route)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        &["Go"],
        &["go"],
        &[r#"\.(?:GET|Handle(?:Func)?)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        &["Rust"],
        &["rs"],
        &[r#"\.(?:get|route)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
    ),
    (
        &["C"],
        &["c", "h"],
        &[
            r#"==\s*"([/\w\-]*health[/\w\-]*)""#,
            r#"strcmp\([^,]*,\s*"([/\w\-]*health[/\w\-]*)""#,
        ],
    ),
    (
        &["C++"],
        &["cpp", "cxx", "cc", "hpp", "h"],
        &[
            r#"==\s*"([/\w\-]*health[/\w\-]*)""#,
            r#"strcmp\([^,]*,\s*"([/\w\-]*health[/\w\-]*)""#,
        ],
    ),
    (
        &["PHP"],
        &["php"],
        &[
            r#"\$app->get\(['"]([/\w\-]*health[/\w\-]*)['"]"#,
            r#"case\s+['"]([/\w\-]*health[/\w\-]*)['"]"#,
            r#"Route::get\(['"]([/\w\-]*health[/\w\-]*)['"]"#,
        ],
    ),
];

/// Language-specific regex patterns for detecting **environment variable** access.
pub const ENV_VAR_PATTERNS: PatternTable = &[
    (
        &["JavaScript", "TypeScript"],
        &["js", "ts", "mjs", "cjs"],
        &[r"process\.env\.([A-Z_][A-Z0-9_]*)"],
    ),
    (
        &["Python"],
        &["py"],
        &[
            r#"os\.environ\.get\(['"]([A-Z_][A-Z0-9_]*)['"]"#,
            r#"os\.getenv\(['"]([A-Z_][A-Z0-9_]*)['"]"#,
            r#"os\.environ\[['"]([A-Z_][A-Z0-9_]*)['"]\]"#,
        ],
    ),
    (&["Rust"], &["rs"], &[r#"env::var\(["']([A-Z_][A-Z0-9_]*)"#]),
    (&["Go"], &["go"], &[r#"os\.Getenv\(["']([A-Z_][A-Z0-9_]*)"#]),
    (
        &["Java", "Kotlin"],
        &["java", "kt", "kts"],
        &[r#"System\.getenv\(["']([A-Z_][A-Z0-9_]*)"#],
    ),
    (
        &["Scala"],
        &["scala", "java"],
        &[
            r#"System\.getenv\(["']([A-Z_][A-Z0-9_]*)"#,
            r#"sys\.env\.get(?:OrElse)?\(["']([A-Z_][A-Z0-9_]*)"#,
        ],
    ),
    (
        &["Elixir"],
        &["ex", "exs"],
        &[r#"System\.get_env\(["']([A-Z_][A-Z0-9_]*)"#],
    ),
    (
        &["C"],
        &["c", "h"],
        &[r#"getenv\(\s*["']([A-Z_][A-Z0-9_]*)["']"#],
    ),
    (
        &["C++"],
        &["cpp", "cxx", "cc", "hpp", "h"],
        &[r#"getenv\(\s*["']([A-Z_][A-Z0-9_]*)["']"#],
    ),
    (
        &["Clojure"],
        &["clj", "cljc", "cljs"],
        &[r#"System/getenv\s+["']([A-Z_][A-Z0-9_]*)"#],
    ),
];

/// Built-in environment variables to skip during env-var scanning.
pub const BUILTIN_ENV_VARS: &[&str] = &["PATH", "HOME", "USER", "SHELL", "LANG", "TERM"];

// ── Generic walker ───────────────────────────────────────────────────────

/// Resolve the pattern table entry for a given language and compile regexes.
///
/// Returns `None` when no entry matches or all regexes fail to compile.
fn resolve_patterns(
    language: &str,
    table: PatternTable,
) -> Option<(&'static [&'static str], Vec<regex::Regex>)> {
    let (_, extensions, patterns) = table
        .iter()
        .find(|(languages, _, _)| languages.contains(&language))?;

    let compiled: Vec<regex::Regex> = patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    if compiled.is_empty() {
        return None;
    }

    Some((extensions, compiled))
}

/// Callback provided to [`walk_source_files`].
///
/// Receives the file content, the list of compiled regexes, and the file
/// path.  Returns `true` to stop walking early.
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

    let (extensions, compiled) = match resolve_patterns(language.as_str(), PORT_PATTERNS) {
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

    // Source-code ports are more specific than framework defaults (they come
    // from explicit bind/listen calls in code), so they go first as the
    // primary port for health checks and port mapping.
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
            // Source-detected ports go first, then existing framework/config ports
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

    let (extensions, compiled) = match resolve_patterns(language.as_str(), HEALTH_PATTERNS) {
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

    let (extensions, compiled) = match resolve_patterns(language.as_str(), ENV_VAR_PATTERNS) {
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
                            build
                                .runtime
                                .env
                                .insert(var_name.to_string(), String::new());
                        }
                    }
                }
            }
            false // keep walking
        },
    );
}

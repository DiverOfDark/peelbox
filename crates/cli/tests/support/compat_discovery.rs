use super::discovery::{has_any_file, Fixture};
use std::fs;
use std::path::{Path, PathBuf};

/// Root of the workspace (where Cargo.lock lives).
fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = Path::new(&manifest_dir);
    // peelbox-cli is at crates/cli — workspace root is two levels up
    manifest_path
        .parent()
        .and_then(|p| p.parent())
        .expect("Could not determine workspace root")
        .to_path_buf()
}

/// Directory where the fetch script deposits sparse clones.
fn external_dir() -> PathBuf {
    workspace_root().join("target").join("external")
}

/// Directory where assembled compat working directories live.
fn compat_work_dir() -> PathBuf {
    workspace_root().join("target").join("compat-work")
}

/// Directory containing committed snapshots.
fn snapshot_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    Path::new(&manifest_dir)
        .join("tests")
        .join("compat-snapshots")
}

/// Discovers compat fixtures from external repos and assembles working directories.
///
/// A compat fixture is included only when BOTH conditions are met:
/// 1. The external project files exist (script was run)
/// 2. A committed snapshot exists in `tests/compat-snapshots/{source}/{example}/`
///
/// Returns an empty vec (with a note) if external fixtures are not present.
#[allow(dead_code)]
pub fn find_compat_fixtures() -> Vec<Fixture> {
    let external = external_dir();
    let snapshots = snapshot_dir();
    let work_root = compat_work_dir();

    if !external.exists() {
        eprintln!(
            "note: external compat fixtures not found at {}. \
             Run ./scripts/fetch-compat-fixtures.sh to enable compat tests.",
            external.display()
        );
        return Vec::new();
    }

    let mut fixtures = Vec::new();

    for source in &["railpack", "nixpacks"] {
        let examples_dir = external.join(source).join("examples");
        let source_snapshots = snapshots.join(source);

        if !examples_dir.exists() {
            eprintln!(
                "note: {}/examples/ not found, skipping {} compat fixtures.",
                source, source
            );
            continue;
        }

        if !source_snapshots.exists() {
            // No committed snapshots for this source yet — nothing to test
            continue;
        }

        let entries = match std::fs::read_dir(&source_snapshots) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let snapshot_entry_path = entry.path();
            if !snapshot_entry_path.is_dir() {
                continue;
            }

            let example_name = entry.file_name().to_string_lossy().into_owned();
            if example_name.starts_with('.') {
                continue;
            }

            // Must have a committed snapshot file
            let snapshot_file = snapshot_entry_path.join("universalbuild.json");
            if !snapshot_file.exists() {
                continue;
            }

            // Must have corresponding external project files
            let external_example = examples_dir.join(&example_name);
            if !external_example.exists() || !has_any_file(&external_example) {
                eprintln!(
                    "note: snapshot exists for compat-{}/{} but external files missing, skipping.",
                    source, example_name
                );
                continue;
            }

            // Check if the snapshot is empty (detection produced no results)
            let snapshot_empty = is_empty_snapshot(&snapshot_file);

            // Assemble working directory
            let category = format!("compat-{}", source);
            let work_dir = work_root.join(&category).join(&example_name);

            if let Err(e) = assemble_compat_work_dir(&external_example, &snapshot_file, &work_dir) {
                eprintln!(
                    "warning: failed to assemble compat work dir for {}/{}: {}",
                    source, example_name, e
                );
                continue;
            }

            fixtures.push(Fixture {
                name: example_name,
                category,
                path: work_dir,
                has_snapshot: true,
                ignore: snapshot_empty,
            });
        }
    }

    fixtures.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));
    fixtures
}

/// Returns true if the snapshot file contains an empty JSON array (`[]`).
fn is_empty_snapshot(path: &Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            trimmed == "[]"
        }
        Err(_) => false,
    }
}

/// Copies external project files + snapshot into a working directory that looks
/// like a normal peelbox fixture.
///
/// Uses an incremental copy approach (no delete-and-recreate) to be safe when
/// multiple test binaries (static_e2e, container_e2e) run in parallel.
fn assemble_compat_work_dir(
    external_example: &Path,
    snapshot_file: &Path,
    work_dir: &Path,
) -> std::io::Result<()> {
    // If the work dir already exists and both the snapshot AND external source
    // files are up to date, skip. We check the external directory's mtime too,
    // because re-fetching external fixtures can add/change files without
    // touching the snapshot.
    let dest_snapshot = work_dir.join("universalbuild.json");
    if work_dir.exists() && dest_snapshot.exists() {
        let snapshot_src_mtime = snapshot_file
            .parent()
            .and_then(newest_mtime_recursive)
            .or_else(|| fs::metadata(snapshot_file).and_then(|m| m.modified()).ok());
        let external_src_mtime = newest_mtime_recursive(external_example);
        let dest_mtime = fs::metadata(&dest_snapshot).and_then(|m| m.modified()).ok();
        if let (Some(d), Some(ss)) = (dest_mtime, snapshot_src_mtime) {
            let snapshot_fresh = d >= ss;
            let external_fresh = external_src_mtime.map(|em| d >= em).unwrap_or(true);
            if snapshot_fresh && external_fresh {
                return Ok(());
            }
        }
    }

    // Copy project files (overwrites existing, never deletes the directory)
    copy_dir_recursive(external_example, work_dir)?;

    // Overlay all files from the snapshot directory (universalbuild.json,
    // expected_output.txt, expected_log.txt, etc.)
    //
    // First, remove snapshot-managed files from the work dir that no longer
    // exist in the snapshot dir (e.g., expected_output.txt was deleted).
    const SNAPSHOT_MANAGED_FILES: &[&str] = &[
        "universalbuild.json",
        "expected_output.txt",
        "expected_log.txt",
    ];
    if let Some(snapshot_dir) = snapshot_file.parent() {
        for filename in SNAPSHOT_MANAGED_FILES {
            let work_file = work_dir.join(filename);
            if work_file.exists() && !snapshot_dir.join(filename).exists() {
                let _ = fs::remove_file(work_file);
            }
        }
        for entry in fs::read_dir(snapshot_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::copy(entry.path(), work_dir.join(entry.file_name()))?;
            }
        }
    }

    // Create a .git marker (peelbox expects it)
    fs::create_dir_all(work_dir.join(".git"))?;

    Ok(())
}

/// Returns the newest modification time found recursively in a directory.
fn newest_mtime_recursive(dir: &Path) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".git" {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    newest = Some(newest.map_or(mtime, |n: std::time::SystemTime| n.max(mtime)));
                }
                if meta.is_dir() {
                    if let Some(child_newest) = newest_mtime_recursive(&entry.path()) {
                        newest =
                            Some(newest.map_or(child_newest, |n: std::time::SystemTime| {
                                n.max(child_newest)
                            }));
                    }
                }
            }
        }
    }
    newest
}

/// Recursively copy a directory, skipping .git directories.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip .git and hidden files from external repos
        if name_str == ".git" || name_str == ".gitignore" {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if ft.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// When PEELBOX_UPDATE_COMPAT_SNAPSHOTS=1, writes detection output as the new
/// snapshot instead of asserting equality.
#[allow(dead_code)]
pub fn should_update_snapshots() -> bool {
    std::env::var("PEELBOX_UPDATE_COMPAT_SNAPSHOTS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Writes a snapshot file for a compat fixture.
#[allow(dead_code)]
pub fn write_compat_snapshot(
    source: &str, // "railpack" or "nixpacks"
    example: &str,
    content: &str,
) -> std::io::Result<()> {
    let dir = snapshot_dir().join(source).join(example);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("universalbuild.json"), content)?;
    Ok(())
}

/// Extracts the compat source name from a category like "compat-railpack".
#[allow(dead_code)]
pub fn parse_compat_category(category: &str) -> Option<&str> {
    category.strip_prefix("compat-")
}

/// Discovers ALL external examples (even without committed snapshots).
/// Used for initial snapshot generation with PEELBOX_UPDATE_COMPAT_SNAPSHOTS=1.
#[allow(dead_code)]
pub fn find_all_external_examples() -> Vec<Fixture> {
    let external = external_dir();
    let work_root = compat_work_dir();

    if !external.exists() {
        return Vec::new();
    }

    let mut fixtures = Vec::new();

    for source in &["railpack", "nixpacks"] {
        let examples_dir = external.join(source).join("examples");
        if !examples_dir.exists() {
            continue;
        }

        let entries = match std::fs::read_dir(&examples_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let example_name = entry.file_name().to_string_lossy().into_owned();
            if example_name.starts_with('.') {
                continue;
            }

            if !has_any_file(&path) {
                continue;
            }

            let category = format!("compat-{}", source);
            let work_dir = work_root.join(&category).join(&example_name);

            // Assemble without requiring a snapshot
            if let Err(e) = assemble_compat_work_dir_no_snapshot(&path, &work_dir) {
                eprintln!(
                    "warning: failed to assemble compat work dir for {}/{}: {}",
                    source, example_name, e
                );
                continue;
            }

            fixtures.push(Fixture {
                name: example_name,
                category,
                path: work_dir,
                // In update mode, treat as having a snapshot so the test runs
                // detection and writes the snapshot via assert_detection_with_mode.
                has_snapshot: true,
                ignore: false,
            });
        }
    }

    fixtures.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));
    fixtures
}

/// Assembles a work dir from external example files only (no snapshot overlay).
fn assemble_compat_work_dir_no_snapshot(
    external_example: &Path,
    work_dir: &Path,
) -> std::io::Result<()> {
    if work_dir.exists() {
        // Already assembled (possibly by find_compat_fixtures with snapshot)
        return Ok(());
    }

    copy_dir_recursive(external_example, work_dir)?;
    std::fs::create_dir_all(work_dir.join(".git"))?;
    Ok(())
}

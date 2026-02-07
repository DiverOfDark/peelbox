pub mod container_harness;
pub mod discovery;
pub mod e2e;

pub use container_harness::ContainerTestHarness;

/// Returns a temporary directory for test artifacts.
///
/// Uses a deterministic name based on the current thread (test) name so that
/// directories are reused across test runs instead of accumulating indefinitely.
/// Callers that need isolated directories should use `TempDir::new_in()` on the
/// returned path.
#[allow(dead_code)]
pub fn get_test_temp_dir() -> std::path::PathBuf {
    let mut target_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if !target_dir.join("Cargo.lock").exists() && !target_dir.join("target").exists() {
        if let Some(parent) = target_dir.parent() {
            if parent.join("Cargo.lock").exists() {
                target_dir = parent.to_path_buf();
            } else if let Some(grandparent) = parent.parent() {
                if grandparent.join("Cargo.lock").exists() {
                    target_dir = grandparent.to_path_buf();
                }
            }
        }
    }
    let thread_name = std::thread::current()
        .name()
        .unwrap_or("unknown")
        .replace("::", "-")
        .replace(['/', '\\'], "-");
    let tmp_dir = target_dir
        .join("target")
        .join("tmp")
        .join(format!("test-{}", thread_name));
    std::fs::create_dir_all(&tmp_dir).expect("Failed to create test temp dir");
    tmp_dir
}

#[allow(dead_code)]
pub fn get_peelbox_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("peelbox")
}

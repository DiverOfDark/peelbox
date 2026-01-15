pub mod container_harness;
pub mod e2e;

pub use container_harness::ContainerTestHarness;

#[allow(dead_code)]
pub fn get_peelbox_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("peelbox")
}

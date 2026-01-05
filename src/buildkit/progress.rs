use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, error, info, warn};

use super::proto::moby::buildkit::v1::{StatusResponse, Vertex, VertexStatus};

/// Log stream type
#[derive(Debug, Copy, Clone)]
enum LogStream {
    Stdout = 1,
    Stderr = 2,
}

impl LogStream {
    fn from_i64(value: i64) -> Self {
        if value == 2 {
            Self::Stderr
        } else {
            Self::Stdout
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    fn log_line(&self, vertex: &str, line: &str) {
        match self {
            Self::Stdout => info!("  [{}:{}] {}", vertex, self.label(), line),
            Self::Stderr => warn!("  [{}:{}] {}", vertex, self.label(), line),
        }
    }
}

/// Mutable state for progress tracking
struct ProgressState {
    /// Map of vertex digest -> Vertex info
    vertices: HashMap<String, Vertex>,
    /// Map of vertex digest -> latest status
    statuses: HashMap<String, VertexStatus>,
    /// Total vertices started
    total_started: usize,
    /// Total vertices cached
    total_cached: usize,
    /// Total vertices completed
    total_completed: usize,
    /// Total vertices errored
    total_errored: usize,
}

impl ProgressState {
    fn new() -> Self {
        Self {
            vertices: HashMap::new(),
            statuses: HashMap::new(),
            total_started: 0,
            total_cached: 0,
            total_completed: 0,
            total_errored: 0,
        }
    }
}

/// Progress tracker for build operations
///
/// Tracks BuildKit vertex states and displays progress to the user.
pub struct ProgressTracker {
    start_time: Instant,
    quiet: bool,
    verbose: bool,
    state: Arc<Mutex<ProgressState>>,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new(quiet: bool, verbose: bool) -> Self {
        Self {
            start_time: Instant::now(),
            quiet,
            verbose,
            state: Arc::new(Mutex::new(ProgressState::new())),
        }
    }

    /// Report build started
    pub fn build_started(&self, image_tag: &str) {
        if !self.quiet {
            info!("Building image {}", image_tag);
        }
        debug!("Build started");
    }

    /// Process a StatusResponse from BuildKit
    pub fn process_status(&self, status: StatusResponse) {
        let mut state = self.state.lock().unwrap();

        // Update vertices
        for vertex in status.vertexes {
            let digest = vertex.digest.clone();

            // Track state changes
            if vertex.started.is_some() && !state.vertices.contains_key(&digest) {
                state.total_started += 1;
                if !self.quiet {
                    info!("[{}] {}", state.total_started, vertex.name);
                }
            }

            if vertex.cached {
                let was_cached = state
                    .vertices
                    .get(&digest)
                    .map(|v| v.cached)
                    .unwrap_or(false);
                if !was_cached {
                    state.total_cached += 1;
                    if !self.quiet {
                        info!("  CACHED {}", vertex.name);
                    }
                }
            }

            if vertex.completed.is_some()
                && state
                    .vertices
                    .get(&digest)
                    .and_then(|v| v.completed.as_ref())
                    .is_none()
            {
                state.total_completed += 1;
                if self.verbose {
                    info!("  DONE {}", vertex.name);
                }
            }

            if !vertex.error.is_empty() {
                let had_error = state
                    .vertices
                    .get(&digest)
                    .map(|v| !v.error.is_empty())
                    .unwrap_or(false);
                if !had_error {
                    state.total_errored += 1;
                    error!("  ERROR {} - {}", vertex.name, vertex.error);
                }
            }

            state.vertices.insert(digest, vertex);
        }

        // Update statuses
        for status_update in status.statuses {
            let vertex_digest = status_update.vertex.clone();
            state
                .statuses
                .insert(status_update.id.clone(), status_update.clone());

            if self.verbose && status_update.total > 0 {
                let vertex_name = state
                    .vertices
                    .get(&vertex_digest)
                    .map(|v| v.name.as_str())
                    .unwrap_or("<unknown>");
                info!(
                    "  {} {} / {} {}",
                    vertex_name, status_update.current, status_update.total, status_update.name
                );
            }
        }

        // Display logs
        for log in status.logs {
            let msg = std::str::from_utf8(&log.msg)
                .map(Cow::Borrowed)
                .unwrap_or_else(|_| String::from_utf8_lossy(&log.msg));

            let vertex_name = state
                .vertices
                .get(&log.vertex)
                .map(|v| v.name.as_str())
                .unwrap_or("<unknown>");

            if !self.quiet {
                let stream = LogStream::from_i64(log.stream);
                for line in msg.lines() {
                    if !line.trim().is_empty() {
                        stream.log_line(vertex_name, line);
                    }
                }
            }

            debug!(
                "Vertex {} log (stream {}) {}",
                log.vertex,
                log.stream,
                msg.trim_end()
            );
        }

        // Display warnings
        for warning in status.warnings {
            let msg = std::str::from_utf8(&warning.short)
                .map(Cow::Borrowed)
                .unwrap_or_else(|_| String::from_utf8_lossy(&warning.short));

            let vertex_name = state
                .vertices
                .get(&warning.vertex)
                .map(|v| v.name.as_str())
                .unwrap_or("<unknown>");

            if !self.quiet {
                warn!("  WARNING [{}] {}", vertex_name, msg.trim_end());
            }
            debug!("Build warning from {} {}", vertex_name, msg.trim_end());
        }
    }

    /// Report build completed
    pub fn build_completed(&self, image_id: &str, size_bytes: u64) {
        let duration = self.start_time.elapsed();
        let state = self.state.lock().unwrap();

        if !self.quiet {
            info!("Build completed in {:.2}s", duration.as_secs_f64());
            info!("  Image ID {}", image_id);
            if size_bytes > 0 {
                info!("  Size {:.2} MB", size_bytes as f64 / 1024.0 / 1024.0);
            }
            info!(
                "  Vertices {} started, {} cached, {} completed, {} errors",
                state.total_started, state.total_cached, state.total_completed, state.total_errored
            );

            if state.total_cached > 0 && state.total_completed > 0 {
                let cache_ratio =
                    (state.total_cached as f64 / state.total_completed as f64) * 100.0;
                info!("  Cache hit ratio {:.1}%", cache_ratio);
            }
        }

        debug!(
            "Build completed in {:?} - {} ({} bytes)",
            duration, image_id, size_bytes
        );
    }

    /// Report build failed
    pub fn build_failed(&self, error: &str) {
        let duration = self.start_time.elapsed();
        let state = self.state.lock().unwrap();

        error!("Build failed after {:.2}s", duration.as_secs_f64());
        error!("  Error {}", error);

        if state.total_errored > 0 {
            error!("  {} vertices reported errors", state.total_errored);
        }

        debug!("Build failed in {:?} - {}", duration, error);
    }
}

/// Build progress event (for future use with async channels)
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    StatusUpdate(StatusResponse),
    BuildCompleted { image_id: String, size_bytes: u64 },
    BuildFailed { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_helpers {
        use super::*;

        pub fn mock_vertex(digest: &str, name: &str) -> Vertex {
            Vertex {
                digest: digest.to_string(),
                name: name.to_string(),
                inputs: vec![],
                cached: false,
                started: Some(prost_types::Timestamp::default()),
                completed: None,
                error: String::new(),
                progress_group: None,
            }
        }

        pub fn mock_cached_vertex(digest: &str, name: &str) -> Vertex {
            Vertex {
                digest: digest.to_string(),
                name: name.to_string(),
                inputs: vec![],
                cached: true,
                started: Some(prost_types::Timestamp::default()),
                completed: Some(prost_types::Timestamp::default()),
                error: String::new(),
                progress_group: None,
            }
        }

        pub fn mock_status(vertices: Vec<Vertex>) -> StatusResponse {
            StatusResponse {
                vertexes: vertices,
                statuses: vec![],
                logs: vec![],
                warnings: vec![],
            }
        }
    }

    use test_helpers::*;

    #[test]
    fn test_progress_tracker_creation() {
        let tracker = ProgressTracker::new(false, false);
        let state = tracker.state.lock().unwrap();
        assert!(!tracker.quiet);
        assert!(!tracker.verbose);
        assert_eq!(state.total_started, 0);
        assert_eq!(state.total_cached, 0);
        assert_eq!(state.total_completed, 0);
        assert_eq!(state.total_errored, 0);
    }

    #[test]
    fn test_quiet_mode() {
        let tracker = ProgressTracker::new(true, false);
        assert!(tracker.quiet);
    }

    #[test]
    fn test_verbose_mode() {
        let tracker = ProgressTracker::new(false, true);
        assert!(tracker.verbose);
    }

    #[test]
    fn test_vertex_tracking() {
        let tracker = ProgressTracker::new(true, false);
        let vertex = mock_vertex("sha256:abc123", "test vertex");
        let status = mock_status(vec![vertex]);

        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_started, 1);
        assert_eq!(state.total_cached, 0);
        assert_eq!(state.total_completed, 0);
    }

    #[test]
    fn test_cached_vertex() {
        let tracker = ProgressTracker::new(true, false);
        let vertex = mock_cached_vertex("sha256:cached", "cached vertex");
        let status = mock_status(vec![vertex]);

        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_started, 1);
        assert_eq!(state.total_cached, 1);
        assert_eq!(state.total_completed, 1);
    }

    #[test]
    fn test_cached_layer_shows_in_normal_mode() {
        let tracker = ProgressTracker::new(false, false);
        let vertex = mock_cached_vertex(
            "sha256:cached123",
            "Load metadata for cgr.dev/chainguard/wolfi-base:latest",
        );
        let status = mock_status(vec![vertex]);

        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_cached, 1, "Cached layer should be counted");
        assert_eq!(state.total_started, 1, "Started count should be incremented");
    }

    #[test]
    fn test_stream_detection() {
        use super::super::proto::moby::buildkit::v1::VertexLog;

        let tracker = ProgressTracker::new(true, false);
        let vertex = mock_vertex("test", "test vertex");

        let status = StatusResponse {
            vertexes: vec![vertex],
            statuses: vec![],
            logs: vec![
                VertexLog {
                    vertex: "test".to_string(),
                    stream: 1, // stdout
                    msg: b"stdout message".to_vec(),
                    timestamp: Some(prost_types::Timestamp::default()),
                },
                VertexLog {
                    vertex: "test".to_string(),
                    stream: 2, // stderr
                    msg: b"stderr message".to_vec(),
                    timestamp: Some(prost_types::Timestamp::default()),
                },
            ],
            warnings: vec![],
        };

        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_started, 1);
    }

    #[test]
    fn test_error_vertex() {
        let tracker = ProgressTracker::new(true, false);
        let mut vertex = mock_vertex("error", "failing task");
        vertex.error = "Build failed".to_string();

        let status = mock_status(vec![vertex]);
        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_errored, 1);
    }

    #[test]
    fn test_malformed_vertex_state() {
        let tracker = ProgressTracker::new(true, false);

        let vertex_no_started = Vertex {
            digest: "no-start".to_string(),
            name: "vertex without started time".to_string(),
            inputs: vec![],
            cached: false,
            started: None,
            completed: None,
            error: String::new(),
            progress_group: None,
        };

        let status = mock_status(vec![vertex_no_started]);
        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(
            state.total_started, 0,
            "Vertex without started time should not count as started"
        );
        drop(state);

        let vertex_completed_no_started = Vertex {
            digest: "completed-no-start".to_string(),
            name: "vertex with completed but no started".to_string(),
            inputs: vec![],
            cached: false,
            started: None,
            completed: Some(prost_types::Timestamp::default()),
            error: String::new(),
            progress_group: None,
        };

        let status = mock_status(vec![vertex_completed_no_started]);
        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(
            state.total_started, 0,
            "Vertex without started time should not count"
        );
        assert_eq!(
            state.total_completed, 1,
            "Vertex with completed should count as completed"
        );
    }

    #[test]
    fn test_vertex_state_transitions() {
        let tracker = ProgressTracker::new(true, false);

        let vertex = mock_vertex("transition", "state transition test");
        let status = mock_status(vec![vertex.clone()]);
        tracker.process_status(status);

        {
            let state = tracker.state.lock().unwrap();
            assert_eq!(state.total_started, 1);
            assert_eq!(state.total_completed, 0);
        }

        let mut vertex_completed = vertex.clone();
        vertex_completed.completed = Some(prost_types::Timestamp::default());
        let status = mock_status(vec![vertex_completed.clone()]);
        tracker.process_status(status);

        {
            let state = tracker.state.lock().unwrap();
            assert_eq!(state.total_started, 1, "Should not double-count started");
            assert_eq!(state.total_completed, 1);
        }

        let status = mock_status(vec![vertex_completed]);
        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(
            state.total_completed, 1,
            "Should not double-count completed"
        );
    }

    #[test]
    fn test_error_state_transition() {
        let tracker = ProgressTracker::new(true, false);

        let vertex = mock_vertex("error-transition", "error transition test");
        let status = mock_status(vec![vertex.clone()]);
        tracker.process_status(status);

        {
            let state = tracker.state.lock().unwrap();
            assert_eq!(state.total_errored, 0);
        }

        let mut vertex_with_error = vertex;
        vertex_with_error.error = "First error".to_string();
        let status = mock_status(vec![vertex_with_error.clone()]);
        tracker.process_status(status);

        {
            let state = tracker.state.lock().unwrap();
            assert_eq!(state.total_errored, 1);
        }

        vertex_with_error.error = "Second error".to_string();
        let status = mock_status(vec![vertex_with_error]);
        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_errored, 1, "Should not double-count errors");
    }

    #[test]
    fn test_multiple_vertices() {
        let tracker = ProgressTracker::new(true, false);

        let vertices = vec![
            mock_vertex("v1", "vertex 1"),
            mock_cached_vertex("v2", "vertex 2"),
            mock_vertex("v3", "vertex 3"),
        ];

        let status = mock_status(vertices);
        tracker.process_status(status);

        let state = tracker.state.lock().unwrap();
        assert_eq!(state.total_started, 3);
        assert_eq!(state.total_cached, 1);
        assert_eq!(state.total_completed, 1);
    }
}

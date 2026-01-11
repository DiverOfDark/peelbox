use anyhow::Result;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info};

use super::call_tracker::CallTracker;
use super::proto::moby::filesync::v1::file_send_server::FileSend as FileSendTrait;
use super::proto::moby::filesync::v1::BytesMessage;

static CALL_TRACKER: CallTracker = CallTracker::new();

#[derive(Debug, Clone)]
pub enum OutputDestination {
    File(PathBuf),
    DockerLoad,
}

impl std::fmt::Display for OutputDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputDestination::File(p) => write!(f, "{}", p.display()),
            OutputDestination::DockerLoad => write!(f, "docker daemon"),
        }
    }
}

/// FileSend gRPC service implementation
///
/// Handles tar export from BuildKit daemon.
/// BuildKit sends the built image as BytesMessage chunks (max 3MB each).
/// This service receives those chunks, assembles them, and writes to output (file or docker load).
pub struct FileSendService {
    destination: Arc<Mutex<OutputDestination>>,
    export_complete_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    bytes_written: Arc<AtomicU64>,
}

impl FileSendService {
    pub fn new(
        destination: OutputDestination,
        export_complete_tx: tokio::sync::oneshot::Sender<()>,
        bytes_written: Arc<AtomicU64>,
    ) -> Self {
        Self {
            destination: Arc::new(Mutex::new(destination)),
            export_complete_tx: Arc::new(Mutex::new(Some(export_complete_tx))),
            bytes_written,
        }
    }
}

#[tonic::async_trait]
impl FileSendTrait for FileSendService {
    type DiffCopyStream = ReceiverStream<Result<BytesMessage, Status>>;

    async fn diff_copy(
        &self,
        request: Request<Streaming<BytesMessage>>,
    ) -> Result<Response<Self::DiffCopyStream>, Status> {
        let call_id = CALL_TRACKER.next_id();
        debug!("FileSend::DiffCopy called - call_id={}", call_id);

        // Extract exporter metadata from request headers
        let metadata = request.metadata();

        // Log all available metadata for debugging
        debug!("FileSend call_id={} request metadata:", call_id);
        for kv in metadata.iter() {
            match kv {
                tonic::metadata::KeyAndValueRef::Ascii(key, value) => {
                    if let Ok(value_str) = value.to_str() {
                        debug!("  {}: {}", key, value_str);
                    }
                }
                tonic::metadata::KeyAndValueRef::Binary(key, _value) => {
                    debug!("  {}: <binary>", key);
                }
            }
        }

        // Extract known exporter parameters
        let exporter_name = metadata.get("exporter").and_then(|v| v.to_str().ok());
        let output_format = metadata.get("format").and_then(|v| v.to_str().ok());
        let compression = metadata.get("compression").and_then(|v| v.to_str().ok());

        debug!(
            "FileSend call_id={} exporter metadata: exporter={:?} format={:?} compression={:?}",
            call_id, exporter_name, output_format, compression
        );

        let mut in_stream = request.into_inner();
        let destination = self.destination.lock().await.clone();
        let export_complete_tx = self.export_complete_tx.clone();
        let bytes_counter = self.bytes_written.clone();

        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            debug!(
                "FileSend call_id={} spawned task started, writing to {}",
                call_id, destination
            );

            // Prepare writer based on destination
            let mut file_writer: Option<File> = None;
            let mut child_process: Option<tokio::process::Child> = None;
            let mut child_stdin: Option<tokio::process::ChildStdin> = None;

            match &destination {
                OutputDestination::File(path) => {
                    // Create parent directories if needed
                    if let Some(parent) = path.parent() {
                        if let Err(e) = tokio::fs::create_dir_all(parent).await {
                            error!(
                                "Failed to create parent directories for {}: {}",
                                path.display(),
                                e
                            );
                            return;
                        }
                    }

                    // Open output file for writing
                    match File::create(path).await {
                        Ok(f) => file_writer = Some(f),
                        Err(e) => {
                            error!("Failed to create output file {}: {}", path.display(), e);
                            return;
                        }
                    };
                }
                OutputDestination::DockerLoad => {
                    info!("Spawning 'docker load' process...");
                    match Command::new("docker")
                        .arg("load")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .spawn()
                    {
                        Ok(mut child) => {
                            child_stdin = child.stdin.take();
                            child_process = Some(child);
                        }
                        Err(e) => {
                            error!("Failed to spawn docker load: {}", e);
                            return;
                        }
                    }
                }
            }

            let mut total_bytes = 0u64;
            let mut chunk_count = 0u64;

            // Receive BytesMessage chunks from BuildKit
            loop {
                match in_stream.message().await {
                    Ok(Some(msg)) => {
                        // Empty BytesMessage signals EOF from client
                        if msg.data.is_empty() {
                            debug!(
                                "FileSend call_id={} received EOF signal (empty chunk)",
                                call_id
                            );
                            break;
                        }

                        chunk_count += 1;
                        let chunk_size = msg.data.len();
                        total_bytes += chunk_size as u64;
                        bytes_counter.store(total_bytes, Ordering::Relaxed);

                        if chunk_count <= 3 || chunk_count % 100 == 0 {
                            debug!(
                                "FileSend call_id={} received chunk #{}: {} bytes (total: {} bytes)",
                                call_id, chunk_count, chunk_size, total_bytes
                            );
                        }

                        // Write chunk to appropriate destination
                        if let Some(w) = file_writer.as_mut() {
                            if let Err(e) = w.write_all(&msg.data).await {
                                error!(
                                    "FileSend call_id={} failed to write to file: {}",
                                    call_id, e
                                );
                                return;
                            }
                        } else if let Some(w) = child_stdin.as_mut() {
                            if let Err(e) = w.write_all(&msg.data).await {
                                error!(
                                    "FileSend call_id={} failed to write to docker load stdin: {}",
                                    call_id, e
                                );
                                return;
                            }
                        }
                    }
                    Ok(None) => {
                        debug!("FileSend call_id={} stream closed by client", call_id);
                        break;
                    }
                    Err(e) => {
                        error!("FileSend call_id={} stream error: {}", call_id, e);
                        return;
                    }
                }
            }

            // Flush and cleanup
            if let Some(mut w) = file_writer {
                if let Err(e) = w.flush().await {
                    error!("FileSend call_id={} failed to flush file: {}", call_id, e);
                    return;
                }
            }

            if let Some(mut w) = child_stdin {
                if let Err(e) = w.shutdown().await {
                    error!(
                        "FileSend call_id={} failed to close docker load stdin: {}",
                        call_id, e
                    );
                }
            }

            if let Some(mut child) = child_process {
                debug!(
                    "FileSend call_id={} waiting for docker load to finish...",
                    call_id
                );
                match child.wait().await {
                    Ok(status) => {
                        if status.success() {
                            info!("Docker load completed successfully");
                        } else {
                            error!("Docker load failed with status: {}", status);
                            return;
                        }
                    }
                    Err(e) => {
                        error!("Failed to wait for docker load: {}", e);
                        return;
                    }
                }
            }

            debug!(
                "FileSend call_id={} export complete: {} chunks, {} bytes written to {}",
                call_id, chunk_count, total_bytes, destination
            );

            // Send empty BytesMessage as ACK (blocking send to ensure delivery)
            let ack = BytesMessage { data: vec![] };
            if let Err(e) = tx.send(Ok(ack)).await {
                error!("FileSend call_id={} failed to send ACK: {}", call_id, e);
                return;
            }

            debug!(
                "FileSend call_id={} sent ACK, waiting for client to close...",
                call_id
            );

            // Wait for client to close their sender (blocking ACK pattern)
            // This ensures the ACK was received before we close our receiver
            match in_stream.message().await {
                Ok(None) => {
                    debug!(
                        "FileSend call_id={} client closed sender after ACK",
                        call_id
                    );
                }
                Ok(Some(msg)) => {
                    debug!(
                        "FileSend call_id={} unexpected message after ACK: {} bytes",
                        call_id,
                        msg.data.len()
                    );
                }
                Err(e) => {
                    debug!(
                        "FileSend call_id={} stream error after ACK (expected): {}",
                        call_id, e
                    );
                }
            }

            drop(tx);
            debug!("FileSend call_id={} complete", call_id);

            // Signal export completion
            if let Some(sender) = export_complete_tx.lock().await.take() {
                debug!("FileSend call_id={} signaling export completion", call_id);
                let _ = sender.send(());
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

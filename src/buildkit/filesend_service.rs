use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error};

use super::call_tracker::CallTracker;
use super::proto::moby::filesync::v1::file_send_server::FileSend as FileSendTrait;
use super::proto::moby::filesync::v1::BytesMessage;

static CALL_TRACKER: CallTracker = CallTracker::new();

/// FileSend gRPC service implementation
///
/// Handles tar export from BuildKit daemon.
/// BuildKit sends the built image as BytesMessage chunks (max 3MB each).
/// This service receives those chunks, assembles them, and writes to output file.
pub struct FileSendService {
    output_path: Arc<Mutex<PathBuf>>,
    export_complete_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl FileSendService {
    pub fn new(output_path: PathBuf, export_complete_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            output_path: Arc::new(Mutex::new(output_path)),
            export_complete_tx: Arc::new(Mutex::new(Some(export_complete_tx))),
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
        let output_path = self.output_path.lock().await.clone();
        let export_complete_tx = self.export_complete_tx.clone();

        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            debug!("FileSend call_id={} spawned task started, writing to {}", call_id, output_path.display());

            // Create parent directories if needed
            if let Some(parent) = output_path.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    error!("Failed to create parent directories for {}: {}", output_path.display(), e);
                    return;
                }
            }

            // Open output file for writing
            let mut file = match File::create(&output_path).await {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create output file {}: {}", output_path.display(), e);
                    return;
                }
            };

            let mut total_bytes = 0u64;
            let mut chunk_count = 0u64;

            // Receive BytesMessage chunks from BuildKit
            loop {
                match in_stream.message().await {
                    Ok(Some(msg)) => {
                        // Empty BytesMessage signals EOF from client
                        if msg.data.is_empty() {
                            debug!("FileSend call_id={} received EOF signal (empty chunk)", call_id);
                            break;
                        }

                        chunk_count += 1;
                        let chunk_size = msg.data.len();
                        total_bytes += chunk_size as u64;

                        if chunk_count <= 3 || chunk_count % 100 == 0 {
                            debug!(
                                "FileSend call_id={} received chunk #{}: {} bytes (total: {} bytes)",
                                call_id, chunk_count, chunk_size, total_bytes
                            );
                        }

                        // Write chunk to file
                        if let Err(e) = file.write_all(&msg.data).await {
                            error!("FileSend call_id={} failed to write chunk: {}", call_id, e);
                            return;
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

            // Flush and close file
            if let Err(e) = file.flush().await {
                error!("FileSend call_id={} failed to flush file: {}", call_id, e);
                return;
            }

            debug!(
                "FileSend call_id={} tar export complete: {} chunks, {} bytes written to {}",
                call_id, chunk_count, total_bytes, output_path.display()
            );

            // Send empty BytesMessage as ACK (blocking send to ensure delivery)
            let ack = BytesMessage { data: vec![] };
            if let Err(e) = tx.send(Ok(ack)).await {
                error!("FileSend call_id={} failed to send ACK: {}", call_id, e);
                return;
            }

            debug!("FileSend call_id={} sent ACK, waiting for client to close...", call_id);

            // Wait for client to close their sender (blocking ACK pattern)
            // This ensures the ACK was received before we close our receiver
            match in_stream.message().await {
                Ok(None) => {
                    debug!("FileSend call_id={} client closed sender after ACK", call_id);
                }
                Ok(Some(msg)) => {
                    debug!("FileSend call_id={} unexpected message after ACK: {} bytes", call_id, msg.data.len());
                }
                Err(e) => {
                    debug!("FileSend call_id={} stream error after ACK (expected): {}", call_id, e);
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

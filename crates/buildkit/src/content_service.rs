use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info, warn};

use super::proto::containerd::services::content::v1::{
    content_server::Content as ContentTrait, AbortRequest, DeleteContentRequest, InfoRequest,
    InfoResponse, ListContentRequest, ListContentResponse, ListStatusesRequest,
    ListStatusesResponse, ReadContentRequest, ReadContentResponse, StatusRequest, StatusResponse,
    UpdateRequest, UpdateResponse, WriteContentRequest, WriteContentResponse,
};

/// Compute blob path from digest and cache directory
fn compute_blob_path(cache_dir: &std::path::Path, digest: &str) -> PathBuf {
    let parts: Vec<&str> = digest.split(':').collect();
    if parts.len() == 2 {
        cache_dir.join("blobs").join(parts[0]).join(parts[1])
    } else {
        cache_dir.join("blobs").join("unknown").join(digest)
    }
}

/// Sanitize ref name for filesystem usage
fn sanitize_ref_name(ref_name: &str) -> String {
    ref_name.replace(['/', ':', '\\'], "_")
}

/// Content service implementation for BuildKit cache export/import
///
/// Implements containerd's Content service protocol to enable:
/// - Cache export: BuildKit writes cache layers to local directory via Write RPC
/// - Cache import: BuildKit reads cache layers from local directory via Read RPC
///
/// The cache directory structure:
/// ```
/// cache_dir/
///   blobs/
///     sha256/
///       <digest> - Content-addressed blob files
///   ingest/
///     <ref> - Temporary files for ongoing writes
/// ```
pub struct ContentService {
    cache_dir: PathBuf,
    /// Track ongoing write operations (ref -> temp file path)
    write_sessions: Arc<Mutex<HashMap<String, WriteSession>>>,
}

struct WriteSession {
    temp_path: PathBuf,
    offset: u64,
}

impl ContentService {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            write_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn blob_path(&self, digest: &str) -> PathBuf {
        compute_blob_path(&self.cache_dir, digest)
    }

    async fn ensure_directories(&self) -> Result<()> {
        let blobs_dir = self.cache_dir.join("blobs").join("sha256");
        let ingest_dir = self.cache_dir.join("ingest");

        debug!(
            "Content::ensure_directories creating: {}",
            blobs_dir.display()
        );
        fs::create_dir_all(&blobs_dir).await?;

        debug!(
            "Content::ensure_directories creating: {}",
            ingest_dir.display()
        );
        fs::create_dir_all(&ingest_dir).await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl ContentTrait for ContentService {
    /// Info returns metadata about a committed content blob
    async fn info(&self, request: Request<InfoRequest>) -> Result<Response<InfoResponse>, Status> {
        let req = request.into_inner();
        let digest = req.digest;

        debug!("Content::Info called for digest={}", digest);

        let blob_path = self.blob_path(&digest);

        match fs::metadata(&blob_path).await {
            Ok(metadata) => {
                let info = super::proto::containerd::services::content::v1::Info {
                    digest: digest.clone(),
                    size: metadata.len() as i64,
                    created_at: None,
                    updated_at: None,
                    labels: HashMap::new(),
                };

                debug!(
                    "Content::Info found blob {} size={}",
                    digest,
                    metadata.len()
                );
                Ok(Response::new(InfoResponse { info: Some(info) }))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!("Content::Info blob not found: {}", digest);
                Err(Status::not_found(format!(
                    "content blob {} not found",
                    digest
                )))
            }
            Err(e) => {
                error!("Content::Info error for {}: {}", digest, e);
                Err(Status::internal(format!("failed to get info: {}", e)))
            }
        }
    }

    /// Update modifies content metadata (labels only)
    async fn update(
        &self,
        _request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        // Not needed for cache operations - labels are rarely updated
        Err(Status::unimplemented("Update not implemented"))
    }

    type ListStream = tokio_stream::wrappers::ReceiverStream<Result<ListContentResponse, Status>>;

    /// List streams all content blobs
    async fn list(
        &self,
        _request: Request<ListContentRequest>,
    ) -> Result<Response<Self::ListStream>, Status> {
        // Not needed for cache import/export - BuildKit knows what it needs
        Err(Status::unimplemented("List not implemented"))
    }

    /// Delete removes a content blob
    async fn delete(
        &self,
        _request: Request<DeleteContentRequest>,
    ) -> Result<Response<()>, Status> {
        // Not needed for cache operations - let OS/GC handle cleanup
        Err(Status::unimplemented("Delete not implemented"))
    }

    type ReadStream = tokio_stream::wrappers::ReceiverStream<Result<ReadContentResponse, Status>>;

    /// Read streams content blob data (used for cache import)
    async fn read(
        &self,
        request: Request<ReadContentRequest>,
    ) -> Result<Response<Self::ReadStream>, Status> {
        let req = request.into_inner();
        debug!(
            "Content::Read called for digest={} offset={} size={}",
            req.digest, req.offset, req.size
        );

        let blob_path = self.blob_path(&req.digest);

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            match fs::File::open(&blob_path).await {
                Ok(mut file) => {
                    // Seek to requested offset
                    if req.offset > 0 {
                        if let Err(e) = file.seek(std::io::SeekFrom::Start(req.offset as u64)).await
                        {
                            let _ = tx
                                .send(Err(Status::internal(format!("seek failed: {}", e))))
                                .await;
                            return;
                        }
                    }

                    // Stream data in chunks
                    let mut buffer = vec![0u8; 65536]; // 64KB chunks
                    let mut remaining = if req.size > 0 {
                        req.size as usize
                    } else {
                        usize::MAX
                    };

                    loop {
                        let to_read = std::cmp::min(buffer.len(), remaining);
                        if to_read == 0 {
                            break;
                        }

                        match file.read(&mut buffer[..to_read]).await {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                let response = ReadContentResponse {
                                    offset: 0, // Not used by BuildKit
                                    data: buffer[..n].to_vec(),
                                };

                                if tx.send(Ok(response)).await.is_err() {
                                    error!("Content::Read channel closed");
                                    return;
                                }

                                remaining -= n;
                            }
                            Err(e) => {
                                error!("Content::Read error: {}", e);
                                let _ = tx
                                    .send(Err(Status::internal(format!("read failed: {}", e))))
                                    .await;
                                return;
                            }
                        }
                    }

                    debug!("Content::Read complete for {}", req.digest);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let _ = tx
                        .send(Err(Status::not_found(format!(
                            "content blob {} not found",
                            req.digest
                        ))))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(Status::internal(format!("open failed: {}", e))))
                        .await;
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    /// Status returns status of an ongoing write operation
    async fn status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        debug!("Content::Status called for ref={}", req.r#ref);

        let sessions = self.write_sessions.lock().await;

        if let Some(session) = sessions.get(&req.r#ref) {
            let status = super::proto::containerd::services::content::v1::Status {
                started_at: None,
                updated_at: None,
                r#ref: req.r#ref.clone(),
                offset: session.offset as i64,
                total: 0,                // Unknown until commit
                expected: String::new(), // Unknown until commit
            };

            Ok(Response::new(StatusResponse {
                status: Some(status),
            }))
        } else {
            Err(Status::not_found(format!(
                "write ref {} not found",
                req.r#ref
            )))
        }
    }

    /// ListStatuses returns status of all ongoing writes
    async fn list_statuses(
        &self,
        _request: Request<ListStatusesRequest>,
    ) -> Result<Response<ListStatusesResponse>, Status> {
        let sessions = self.write_sessions.lock().await;

        let statuses = sessions
            .iter()
            .map(
                |(ref_name, session)| super::proto::containerd::services::content::v1::Status {
                    started_at: None,
                    updated_at: None,
                    r#ref: ref_name.clone(),
                    offset: session.offset as i64,
                    total: 0,
                    expected: String::new(),
                },
            )
            .collect();

        Ok(Response::new(ListStatusesResponse { statuses }))
    }

    /// Write handles bidirectional streaming for content writes (used for cache export)
    async fn write(
        &self,
        request: Request<Streaming<WriteContentRequest>>,
    ) -> Result<Response<Self::WriteStream>, Status> {
        debug!(
            "Content::Write called (bidirectional stream), cache_dir={}",
            self.cache_dir.display()
        );

        if let Err(e) = self.ensure_directories().await {
            error!("Failed to create cache directories: {}", e);
            return Err(Status::internal(format!(
                "failed to create cache directories: {}",
                e
            )));
        }

        let mut in_stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let write_sessions = self.write_sessions.clone();
        let cache_dir = self.cache_dir.clone();

        tokio::spawn(async move {
            let mut current_ref: Option<String> = None;
            let mut current_file: Option<tokio::fs::File> = None;
            let mut current_offset = 0u64;

            while let Ok(Some(req)) = in_stream.message().await {
                let ref_name = req.r#ref.clone();

                // Handle requests with empty ref (continuation of current write session)
                if ref_name.is_empty() {
                    match req.action {
                        0 => {
                            // STAT: Query status of current write
                            debug!("Content::Write received STAT request for current session");
                            let response = WriteContentResponse {
                                action: 0, // STAT
                                offset: current_offset as i64,
                                total: req.total,
                                digest: String::new(),
                                started_at: None,
                                updated_at: None,
                            };

                            if tx.send(Ok(response)).await.is_err() {
                                error!("Content::Write channel closed");
                                return;
                            }
                            continue;
                        }
                        1 => {
                            // WRITE: Continue writing to current file
                            debug!(
                                "Content::Write received WRITE continuation for current session"
                            );
                            // Fall through to normal WRITE handling below
                            // Use current_ref as ref_name
                            if current_ref.is_none() {
                                warn!("Content::Write received WRITE with empty ref but no current session");
                                continue;
                            }
                            // Don't skip - process as normal WRITE for current session
                        }
                        2 => {
                            // COMMIT: Finalize current write
                            debug!("Content::Write received COMMIT for current session");
                            // Fall through to COMMIT handling
                        }
                        _ => {
                            warn!("Content::Write received empty ref name with unknown action={}, skipping", req.action);
                            continue;
                        }
                    }
                }

                // Initialize new write session if this is a new ref (skip if empty ref continuation)
                if !ref_name.is_empty() && current_ref.as_ref() != Some(&ref_name) {
                    // Close previous file if exists
                    if let Some(file) = current_file.take() {
                        drop(file);
                    }

                    current_ref = Some(ref_name.clone());
                    current_offset = 0;

                    let ingest_path = cache_dir.join("ingest").join(sanitize_ref_name(&ref_name));

                    debug!(
                        "Content::Write creating ingest file at: {}",
                        ingest_path.display()
                    );

                    match tokio::fs::File::create(&ingest_path).await {
                        Ok(file) => {
                            debug!("Content::Write started for ref={}", ref_name);
                            current_file = Some(file);

                            let mut sessions = write_sessions.lock().await;
                            sessions.insert(
                                ref_name.clone(),
                                WriteSession {
                                    temp_path: ingest_path.clone(),
                                    offset: 0,
                                },
                            );
                        }
                        Err(e) => {
                            error!("Content::Write failed to create ingest file: {}", e);
                            let _ = tx
                                .send(Err(Status::internal(format!(
                                    "failed to create ingest file: {}",
                                    e
                                ))))
                                .await;
                            return;
                        }
                    }
                }

                // Use current_ref if ref_name is empty (continuation)
                let effective_ref = if ref_name.is_empty() {
                    current_ref.clone().unwrap_or_default()
                } else {
                    ref_name.clone()
                };

                // Handle write action
                match req.action {
                    1 => {
                        // WRITE: Write data at offset
                        if let Some(file) = current_file.as_mut() {
                            if !req.data.is_empty() {
                                match file.write_all(&req.data).await {
                                    Ok(_) => {
                                        current_offset += req.data.len() as u64;

                                        // Update session offset
                                        let mut sessions = write_sessions.lock().await;
                                        if let Some(session) = sessions.get_mut(&effective_ref) {
                                            session.offset = current_offset;
                                        }

                                        // Send response
                                        let response = WriteContentResponse {
                                            action: 1, // WRITE
                                            offset: current_offset as i64,
                                            total: req.total,
                                            digest: String::new(),
                                            started_at: None,
                                            updated_at: None,
                                        };

                                        if tx.send(Ok(response)).await.is_err() {
                                            error!("Content::Write channel closed");
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        error!("Content::Write failed: {}", e);
                                        let _ = tx
                                            .send(Err(Status::internal(format!(
                                                "write failed: {}",
                                                e
                                            ))))
                                            .await;
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    0 => {
                        // STAT: Return current status (hold write lock)
                        let response = WriteContentResponse {
                            action: 0, // STAT
                            offset: current_offset as i64,
                            total: req.total,
                            digest: String::new(),
                            started_at: None,
                            updated_at: None,
                        };

                        if tx.send(Ok(response)).await.is_err() {
                            error!("Content::Write channel closed");
                            return;
                        }
                    }
                    2 => {
                        // COMMIT: Finalize write and move to blob storage
                        if let Some(file) = current_file.take() {
                            if let Err(e) = file.sync_all().await {
                                error!("Content::Write sync failed: {}", e);
                                let _ = tx
                                    .send(Err(Status::internal(format!("sync failed: {}", e))))
                                    .await;
                                return;
                            }
                            drop(file);

                            // Move from ingest to blob storage
                            let digest = req.expected.clone();
                            let blob_path = compute_blob_path(&cache_dir, &digest);

                            if let Some(parent) = blob_path.parent() {
                                let _ = tokio::fs::create_dir_all(parent).await;
                            }

                            let ingest_path = cache_dir
                                .join("ingest")
                                .join(sanitize_ref_name(&effective_ref));

                            debug!(
                                "Content::Write committing: rename {} -> {}",
                                ingest_path.display(),
                                blob_path.display()
                            );

                            match tokio::fs::rename(&ingest_path, &blob_path).await {
                                Ok(_) => {
                                    info!(
                                        "Content::Write COMMITTED: ref='{}' digest='{}' size={} path={}",
                                        effective_ref, digest, current_offset, blob_path.display()
                                    );

                                    // Remove from write sessions before sending response
                                    {
                                        let mut sessions = write_sessions.lock().await;
                                        sessions.remove(&effective_ref);
                                    }

                                    // Send commit response
                                    let response = WriteContentResponse {
                                        action: 2, // COMMIT
                                        offset: current_offset as i64,
                                        total: current_offset as i64,
                                        digest: digest.clone(),
                                        started_at: None,
                                        updated_at: None,
                                    };

                                    if tx.send(Ok(response)).await.is_err() {
                                        error!("Content::Write channel closed");
                                        return;
                                    }
                                }
                                Err(e) => {
                                    error!("Content::Write commit failed: {}", e);
                                    let _ = tx
                                        .send(Err(Status::internal(format!(
                                            "commit failed: {}",
                                            e
                                        ))))
                                        .await;
                                    return;
                                }
                            }
                        }

                        current_ref = None;
                    }
                    _ => {
                        warn!("Content::Write unknown action: {}", req.action);
                    }
                }
            }

            debug!("Content::Write stream completed");
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    type WriteStream = tokio_stream::wrappers::ReceiverStream<Result<WriteContentResponse, Status>>;

    /// Abort cancels an ongoing write operation
    async fn abort(&self, request: Request<AbortRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Content::Abort called for ref={}", req.r#ref);

        let mut sessions = self.write_sessions.lock().await;

        if let Some(session) = sessions.remove(&req.r#ref) {
            // Delete temp file
            let _ = fs::remove_file(&session.temp_path).await;
            debug!("Content::Abort removed temp file for ref={}", req.r#ref);
        }

        Ok(Response::new(()))
    }
}

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::digest::{blob_path_or_fallback, Digest};
use super::proto::containerd::services::content::v1::{
    content_server::Content as ContentTrait, AbortRequest, DeleteContentRequest, InfoRequest,
    InfoResponse, ListContentRequest, ListContentResponse, ListStatusesRequest,
    ListStatusesResponse, ReadContentRequest, ReadContentResponse, StatusRequest, StatusResponse,
    UpdateRequest, UpdateResponse, WriteContentRequest, WriteContentResponse,
};

const STREAM_BUFFER_SIZE: usize = 64 * 1024; // 64KB

// WriteAction enum values as constants for pattern matching
const ACTION_STAT: i32 = 0;
const ACTION_WRITE: i32 = 1;
const ACTION_COMMIT: i32 = 2;

/// Content service implementation for BuildKit cache export/import
///
/// Implements containerd's Content service protocol to enable:
/// - Cache export: BuildKit writes cache layers to local directory via Write RPC
/// - Cache import: BuildKit reads cache layers from local directory via Read RPC
pub struct ContentService {
    cache_dir: PathBuf,
    write_sessions: Arc<Mutex<HashMap<String, WriteSession>>>,
    pub last_committed_digest: Arc<Mutex<Option<String>>>,
}

struct WriteSession {
    temp_path: PathBuf,
    offset: u64,
}

impl WriteSession {
    fn to_status(&self, ref_name: &str) -> super::proto::containerd::services::content::v1::Status {
        super::proto::containerd::services::content::v1::Status {
            started_at: None,
            updated_at: None,
            r#ref: ref_name.to_string(),
            offset: self.offset as i64,
            total: 0,
            expected: String::new(),
        }
    }
}

impl ContentService {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            write_sessions: Arc::new(Mutex::new(HashMap::new())),
            last_committed_digest: Arc::new(Mutex::new(None)),
        }
    }

    fn blob_path(&self, digest: &str) -> PathBuf {
        blob_path_or_fallback(digest, &self.cache_dir)
    }

    /// Sanitize reference name for use as a filesystem path component
    fn sanitize_ref(ref_name: &str) -> String {
        ref_name.replace(['/', ':', '\\'], "_")
    }

    async fn ensure_directories(&self) -> Result<()> {
        let dirs = [
            self.cache_dir.join("blobs/sha256"),
            self.cache_dir.join("ingest"),
        ];

        for dir in &dirs {
            fs::create_dir_all(dir).await?;
        }

        debug!("Created cache directories: {:?}", dirs);
        Ok(())
    }

    pub async fn gc(&self) -> Result<()> {
        let last_digest = {
            let guard = self.last_committed_digest.lock().await;
            guard.clone()
        };

        let cache_dir = self.cache_dir.clone();
        tokio::task::spawn_blocking(move || {
            let index = crate::OciIndex::read_with_lock(&cache_dir)?;
            let mut reachable = index.get_reachable_digests(&cache_dir);

            if let Some(digest) = last_digest {
                reachable.insert(digest);
            }

            crate::OciIndex::gc(&cache_dir, &reachable)?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("GC task panicked")??;

        Ok(())
    }
}

#[tonic::async_trait]
impl ContentTrait for ContentService {
    async fn info(&self, request: Request<InfoRequest>) -> Result<Response<InfoResponse>, Status> {
        let req = request.into_inner();
        let digest = req.digest;

        debug!("Content::Info digest={}", digest);

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

                debug!("Content::Info found {} size={}", digest, metadata.len());
                Ok(Response::new(InfoResponse { info: Some(info) }))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!("Content::Info not found: {}", digest);
                Err(Status::not_found(format!(
                    "content blob not found: {}",
                    digest
                )))
            }
            Err(e) => {
                error!("Content::Info error {}: {}", digest, e);
                Err(Status::internal(format!("failed to get blob info: {}", e)))
            }
        }
    }

    async fn update(
        &self,
        _request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        Err(Status::unimplemented("Update not implemented"))
    }

    type ListStream = tokio_stream::wrappers::ReceiverStream<Result<ListContentResponse, Status>>;

    async fn list(
        &self,
        _request: Request<ListContentRequest>,
    ) -> Result<Response<Self::ListStream>, Status> {
        Err(Status::unimplemented("List not implemented"))
    }

    async fn delete(
        &self,
        _request: Request<DeleteContentRequest>,
    ) -> Result<Response<()>, Status> {
        Err(Status::unimplemented("Delete not implemented"))
    }

    type ReadStream = tokio_stream::wrappers::ReceiverStream<Result<ReadContentResponse, Status>>;

    async fn read(
        &self,
        request: Request<ReadContentRequest>,
    ) -> Result<Response<Self::ReadStream>, Status> {
        let req = request.into_inner();
        debug!(
            "Content::Read digest={} offset={} size={}",
            req.digest, req.offset, req.size
        );

        let blob_path = self.blob_path(&req.digest);
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            match fs::File::open(&blob_path).await {
                Ok(mut file) => {
                    if req.offset > 0 {
                        if let Err(e) = file.seek(std::io::SeekFrom::Start(req.offset as u64)).await
                        {
                            let _ = tx
                                .send(Err(Status::internal(format!("seek failed: {}", e))))
                                .await;
                            return;
                        }
                    }

                    let mut buffer = vec![0u8; STREAM_BUFFER_SIZE];
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
                            Ok(0) => break,
                            Ok(n) => {
                                let response = ReadContentResponse {
                                    offset: 0,
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
                            "content blob not found: {}",
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

    async fn status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        debug!("Content::Status ref={}", req.r#ref);

        let sessions = self.write_sessions.lock().await;

        if let Some(session) = sessions.get(&req.r#ref) {
            let status = session.to_status(&req.r#ref);

            Ok(Response::new(StatusResponse {
                status: Some(status),
            }))
        } else {
            Err(Status::not_found(format!(
                "write session not found: {}",
                req.r#ref
            )))
        }
    }

    async fn list_statuses(
        &self,
        _request: Request<ListStatusesRequest>,
    ) -> Result<Response<ListStatusesResponse>, Status> {
        let sessions = self.write_sessions.lock().await;

        let statuses = sessions
            .iter()
            .map(|(ref_name, session)| session.to_status(ref_name))
            .collect();

        Ok(Response::new(ListStatusesResponse { statuses }))
    }

    type WriteStream = tokio_stream::wrappers::ReceiverStream<Result<WriteContentResponse, Status>>;

    async fn write(
        &self,
        request: Request<Streaming<WriteContentRequest>>,
    ) -> Result<Response<Self::WriteStream>, Status> {
        debug!("Content::Write cache_dir={}", self.cache_dir.display());

        self.ensure_directories().await.map_err(|e| {
            error!("Failed to create cache directories: {}", e);
            Status::internal(format!("failed to create cache directories: {}", e))
        })?;

        let mut in_stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let cache_dir = self.cache_dir.clone();
        let write_sessions = self.write_sessions.clone();
        let last_committed_digest = self.last_committed_digest.clone();

        tokio::spawn(async move {
            let mut manager =
                WriteSessionManager::new(cache_dir, write_sessions, last_committed_digest);

            while let Ok(Some(req)) = in_stream.message().await {
                let result = manager.handle_request(req).await;

                match result {
                    Ok(Some(response)) => {
                        if tx.send(Ok(response)).await.is_err() {
                            error!("Content::Write channel closed");
                            return;
                        }
                    }
                    Ok(None) => continue,
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                }
            }

            debug!("Content::Write stream completed");
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn abort(&self, request: Request<AbortRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        debug!("Content::Abort ref={}", req.r#ref);

        let mut sessions = self.write_sessions.lock().await;

        if let Some(session) = sessions.remove(&req.r#ref) {
            let _ = fs::remove_file(&session.temp_path).await;
            debug!("Content::Abort removed temp file for ref={}", req.r#ref);
        }

        Ok(Response::new(()))
    }
}

struct WriteSessionManager {
    cache_dir: PathBuf,
    sessions: Arc<Mutex<HashMap<String, WriteSession>>>,
    last_committed_digest: Arc<Mutex<Option<String>>>,
    current_ref: Option<String>,
    current_file: Option<tokio::fs::File>,
    current_ingest_path: Option<PathBuf>,
    offset: u64,
}

impl WriteSessionManager {
    fn new(
        cache_dir: PathBuf,
        sessions: Arc<Mutex<HashMap<String, WriteSession>>>,
        last_committed_digest: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            cache_dir,
            sessions,
            last_committed_digest,
            current_ref: None,
            current_file: None,
            current_ingest_path: None,
            offset: 0,
        }
    }

    async fn handle_request(
        &mut self,
        req: WriteContentRequest,
    ) -> Result<Option<WriteContentResponse>, Status> {
        let ref_name = req.r#ref.clone();

        if ref_name.is_empty() {
            return self.handle_continuation(req).await;
        }

        if self.current_ref.as_ref() != Some(&ref_name) {
            self.start_write(ref_name).await?;
        }

        self.handle_action(req).await
    }

    async fn handle_continuation(
        &mut self,
        req: WriteContentRequest,
    ) -> Result<Option<WriteContentResponse>, Status> {
        match req.action {
            ACTION_STAT => Ok(Some(self.build_response(
                ACTION_STAT,
                req.total,
                String::new(),
            ))),
            ACTION_WRITE => {
                if self.current_ref.is_none() {
                    warn!("WRITE with empty ref but no session");
                    return Ok(None);
                }
                self.handle_action(req).await
            }
            ACTION_COMMIT => self.handle_action(req).await,
            _ => {
                warn!("Empty ref with unknown action={}", req.action);
                Ok(None)
            }
        }
    }

    async fn start_write(&mut self, ref_name: String) -> Result<(), Status> {
        if let Some(file) = self.current_file.take() {
            drop(file);
        }

        // Generate a unique suffix to avoid collisions if multiple streams
        // upload the same ref concurrently
        let unique_suffix = Uuid::new_v4().to_string();
        let sanitized_ref = ContentService::sanitize_ref(&ref_name);

        let ingest_path = self
            .cache_dir
            .join("ingest")
            .join(format!("{}_{}", sanitized_ref, unique_suffix));

        debug!("Starting write: {}", ingest_path.display());

        let file = tokio::fs::File::create(&ingest_path).await.map_err(|e| {
            error!("Failed to create ingest file: {}", e);
            Status::internal(format!("failed to create ingest file: {}", e))
        })?;

        self.current_file = Some(file);
        self.current_ref = Some(ref_name.clone());
        self.current_ingest_path = Some(ingest_path.clone());
        self.offset = 0;

        let mut sessions = self.sessions.lock().await;
        sessions.insert(
            ref_name,
            WriteSession {
                temp_path: ingest_path,
                offset: 0,
            },
        );

        Ok(())
    }

    async fn handle_action(
        &mut self,
        req: WriteContentRequest,
    ) -> Result<Option<WriteContentResponse>, Status> {
        match req.action {
            ACTION_WRITE => self.write_data(req).await,
            ACTION_STAT => Ok(Some(self.build_response(
                ACTION_STAT,
                req.total,
                String::new(),
            ))),
            ACTION_COMMIT => self.commit(req).await.map(Some),
            _ => {
                warn!("Unknown action: {}", req.action);
                Ok(None)
            }
        }
    }

    async fn write_data(
        &mut self,
        req: WriteContentRequest,
    ) -> Result<Option<WriteContentResponse>, Status> {
        if req.data.is_empty() {
            return Ok(None);
        }

        let file = self
            .current_file
            .as_mut()
            .ok_or_else(|| Status::failed_precondition("no active write session"))?;

        file.write_all(&req.data).await.map_err(|e| {
            error!("Write failed: {}", e);
            Status::internal(format!("write failed: {}", e))
        })?;

        self.offset += req.data.len() as u64;

        if let Some(ref ref_name) = self.current_ref {
            let mut sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get_mut(ref_name) {
                session.offset = self.offset;
            }
        }

        Ok(Some(self.build_response(
            ACTION_WRITE,
            req.total,
            String::new(),
        )))
    }

    fn build_response(&self, action: i32, total: i64, digest: String) -> WriteContentResponse {
        WriteContentResponse {
            action,
            offset: self.offset as i64,
            total,
            digest,
            started_at: None,
            updated_at: None,
        }
    }

    async fn commit(&mut self, req: WriteContentRequest) -> Result<WriteContentResponse, Status> {
        let file = self
            .current_file
            .take()
            .ok_or_else(|| Status::failed_precondition("no active write session to commit"))?;

        file.sync_all().await.map_err(|e| {
            error!("Sync failed: {}", e);
            Status::internal(format!("sync failed: {}", e))
        })?;
        drop(file);

        let digest = req.expected.clone();
        let blob_path = Digest::parse(&digest)
            .map(|d| d.to_blob_path(&self.cache_dir))
            .unwrap_or_else(|_| self.cache_dir.join("blobs/unknown").join(&digest));

        if let Some(parent) = blob_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                error!("Failed to create blob parent directory: {}", e);
                Status::internal(format!("failed to create blob parent directory: {}", e))
            })?;
        }

        let ref_name = self.current_ref.take().unwrap_or_default();

        // Use the path stored in the session manager, ensuring we reference the
        // unique file created in start_write
        let ingest_path = self
            .current_ingest_path
            .take()
            .ok_or_else(|| Status::internal("ingest path missing from session manager"))?;

        debug!(
            "Committing: {} -> {}",
            ingest_path.display(),
            blob_path.display()
        );

        if !ingest_path.exists() {
            error!(
                "Ingest path missing before rename: {}",
                ingest_path.display()
            );
        }

        tokio::fs::rename(&ingest_path, &blob_path)
            .await
            .map_err(|e| {
                error!("Commit failed: {}", e);
                Status::internal(format!("commit failed: {}", e))
            })?;

        info!(
            "COMMITTED: ref='{}' digest='{}' size={} path={}",
            ref_name,
            digest,
            self.offset,
            blob_path.display()
        );

        {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(&ref_name);
        }

        {
            let mut last_digest = self.last_committed_digest.lock().await;
            *last_digest = Some(digest.clone());
        }

        Ok(self.build_response(ACTION_COMMIT, self.offset as i64, digest))
    }
}

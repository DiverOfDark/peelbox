use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error};

use super::call_tracker::CallTracker;
use super::filesync::FileSync;
use super::fsutil;
use super::proto::fsutil::types::{packet::PacketType, Packet, Stat};
use super::proto::moby::filesync::v1::file_sync_server::FileSync as FileSyncTrait;

static CALL_TRACKER: CallTracker = CallTracker::new();

/// FileSync gRPC service implementation
///
/// Handles bidirectional streaming for file transfer to BuildKit daemon.
/// Implements the fsutil packet protocol in push mode:
/// 1. For each file, send PACKET_STAT (metadata) followed immediately by PACKET_DATA (content)
/// 2. After all files, send PACKET_FIN to signal completion
/// 3. Send PACKET_ERR on errors
///
/// Uses a mutex to serialize DiffCopy calls and prevent packet interleaving
/// when BuildKit makes concurrent requests.
pub struct FileSyncService {
    file_sync: Arc<FileSync>,
    /// Mutex to ensure only one DiffCopy call is active at a time
    /// Prevents packet interleaving from concurrent BuildKit requests
    diff_copy_lock: Arc<Mutex<()>>,
}

impl FileSyncService {
    pub fn new(context_path: PathBuf) -> Self {
        Self {
            file_sync: Arc::new(FileSync::new(&context_path)),
            diff_copy_lock: Arc::new(Mutex::new(())),
        }
    }
}

#[tonic::async_trait]
impl FileSyncTrait for FileSyncService {
    type DiffCopyStream = ReceiverStream<Result<Packet, Status>>;
    type TarStreamStream = ReceiverStream<Result<Packet, Status>>;

    async fn diff_copy(
        &self,
        request: Request<Streaming<Packet>>,
    ) -> Result<Response<Self::DiffCopyStream>, Status> {
        let call_id = CALL_TRACKER.next_id();
        debug!("FileSync::DiffCopy called - call_id={}", call_id);

        // Extract metadata from request headers (BuildKit sends dir-name, patterns, etc.)
        let metadata = request.metadata();
        let dir_name = metadata.get("dir-name").map(|v| v.to_str().unwrap_or(""));
        let include_patterns: Vec<String> = metadata
            .get_all("include-patterns")
            .iter()
            .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
            .collect();
        let exclude_patterns: Vec<String> = metadata
            .get_all("exclude-patterns")
            .iter()
            .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
            .collect();

        debug!(
            "DiffCopy call_id={} metadata: dir_name={:?} include={:?} exclude={:?}",
            call_id, dir_name, include_patterns, exclude_patterns
        );

        let mut in_stream = request.into_inner();
        let file_sync = self.file_sync.clone();
        let diff_copy_lock = self.diff_copy_lock.clone();

        let (tx, rx) = mpsc::channel(1000); // Larger buffer to prevent blocking

        tokio::spawn(async move {
            debug!("DiffCopy call_id={} spawned task started", call_id);
            // Acquire lock to serialize DiffCopy calls and prevent packet interleaving
            let _lock = diff_copy_lock.lock().await;
            debug!(
                "DiffCopy call_id={} lock acquired - starting file transfer",
                call_id
            );

            // Step 1: Scan files
            let file_stats = match file_sync.scan_files().await {
                Ok(stats) => {
                    debug!(
                        "DiffCopy call_id={} scanned {} files to transfer",
                        call_id,
                        stats.len()
                    );
                    stats
                }
                Err(e) => {
                    error!("DiffCopy call_id={} failed to scan files: {}", call_id, e);
                    let err_packet = Packet {
                        r#type: PacketType::PacketErr as i32,
                        stat: None,
                        id: 0,
                        data: format!("Failed to scan files: {}", e).into_bytes(),
                    };
                    let _ = tx.send(Ok(err_packet)).await;
                    return;
                }
            };

            // Step 2: Build files map (ID -> path) for regular files only
            //
            // CRITICAL ALGORITHM: ID Assignment Logic
            // ========================================
            // IDs are assigned by PACKET_STAT index (0, 1, 2, ...), NOT file index!
            // This matches Go's fsutil.Send() behavior exactly:
            //
            // - Sender increments ID for EVERY PACKET_STAT sent (dirs, files, symlinks)
            // - Sender only adds regular files to the files map
            // - Directories and symlinks get IDs but cannot request data via PACKET_REQ
            //
            // Example:
            //   Packet 0: Directory "src/" -> ID 0 (not in files map)
            //   Packet 1: File "main.rs" -> ID 1 (added to files map)
            //   Packet 2: Symlink "link" -> ID 2 (not in files map)
            //   Packet 3: File "lib.rs" -> ID 3 (added to files map)
            //
            // When BuildKit sends PACKET_REQ with id=3, we look up files_map[3] = "lib.rs"
            //
            // Go FileMode Format (from Go's fs package):
            // - Directories: 0x80000000 | perms (bit 31 set, ModeDir)
            // - Symlinks: 0x08000000 | perms (bit 27 set, ModeSymlink)
            // - Regular files: just perms (no special bits set)
            //
            // References:
            // - Go fsutil: github.com/tonistiigi/fsutil/send.go
            // - Go fs.FileMode: golang.org/pkg/io/fs/#FileMode
            let mut files_map = std::collections::HashMap::new();
            for (packet_id, file_stat) in file_stats.iter().enumerate() {
                let is_regular_file = fsutil::is_regular_file(
                    file_stat.mode,
                    file_stat.is_dir,
                    file_stat.linkname.is_some(),
                );

                if is_regular_file {
                    files_map.insert(packet_id as u32, file_stat.path.clone());
                }
            }
            debug!(
                "DiffCopy call_id={} built files map with {} regular files out of {} total packets",
                call_id,
                files_map.len(),
                file_stats.len()
            );

            // Step 3: Send all PACKET_STAT (metadata) WITHOUT setting ID field
            debug!(
                "DiffCopy call_id={} sending PACKET_STAT for {} files (PULL mode)",
                call_id,
                file_stats.len()
            );
            for (index, file_stat) in file_stats.iter().enumerate() {
                let path_string = file_stat.path.to_string_lossy().to_string();

                if index < 10 {
                    debug!(
                        "DiffCopy call_id={} file #{}: path='{}' is_dir={} mode=0x{:x} size={}",
                        call_id,
                        index,
                        path_string,
                        file_stat.is_dir,
                        file_stat.mode,
                        file_stat.size
                    );
                }

                let stat = Stat {
                    path: path_string.clone(),
                    mode: file_stat.mode,
                    uid: file_stat.uid,
                    gid: file_stat.gid,
                    size: if file_stat.is_dir {
                        0
                    } else {
                        file_stat.size as i64
                    },
                    mod_time: file_stat.mod_time,
                    linkname: file_stat.linkname.clone().unwrap_or_default(),
                    devmajor: 0,
                    devminor: 0,
                    xattrs: Default::default(),
                };

                // CRITICAL: ID field MUST be 0 (not set)
                // Receiver assigns IDs implicitly based on packet order
                let stat_packet = Packet {
                    r#type: PacketType::PacketStat as i32,
                    stat: Some(stat),
                    id: 0,
                    data: vec![],
                };

                if tx.send(Ok(stat_packet)).await.is_err() {
                    error!(
                        "DiffCopy call_id={} failed to send PACKET_STAT - channel closed",
                        call_id
                    );
                    return;
                }
            }

            // Send final empty PACKET_STAT to signal end of metadata
            debug!(
                "DiffCopy call_id={} sent all PACKET_STAT, sending final empty PACKET_STAT",
                call_id
            );
            let final_stat_packet = Packet {
                r#type: PacketType::PacketStat as i32,
                stat: None,
                id: 0,
                data: vec![],
            };

            if tx.send(Ok(final_stat_packet)).await.is_err() {
                error!(
                    "DiffCopy call_id={} failed to send final PACKET_STAT - channel closed",
                    call_id
                );
                return;
            }

            debug!(
                "DiffCopy call_id={} waiting for PACKET_REQ from BuildKit receiver",
                call_id
            );

            // Step 4: Wait for PACKET_REQ from BuildKit and respond with PACKET_DATA
            loop {
                match in_stream.message().await {
                    Ok(Some(packet)) => {
                        let packet_type = PacketType::try_from(packet.r#type).ok();
                        debug!(
                            "DiffCopy call_id={} received: type={:?} id={}",
                            call_id, packet_type, packet.id
                        );

                        match packet_type {
                            Some(PacketType::PacketReq) => {
                                // BuildKit requests file data
                                let req_id = packet.id;
                                debug!(
                                    "DiffCopy call_id={} received PACKET_REQ for id={}",
                                    call_id, req_id
                                );

                                let file_path = match files_map.get(&req_id) {
                                    Some(path) => path.clone(),
                                    None => {
                                        error!(
                                            "DiffCopy call_id={} invalid file request id={}",
                                            call_id, req_id
                                        );
                                        let err_packet = Packet {
                                            r#type: PacketType::PacketErr as i32,
                                            stat: None,
                                            id: req_id,
                                            data: format!("Invalid file request {}", req_id)
                                                .into_bytes(),
                                        };
                                        let _ = tx.send(Ok(err_packet)).await;
                                        return;
                                    }
                                };

                                // Read and send file chunks
                                match file_sync.read_file_chunks(&file_path).await {
                                    Ok(chunks) => {
                                        for chunk in chunks {
                                            let data_packet = Packet {
                                                r#type: PacketType::PacketData as i32,
                                                stat: None,
                                                id: req_id,
                                                data: chunk,
                                            };

                                            if tx.send(Ok(data_packet)).await.is_err() {
                                                error!("DiffCopy call_id={} failed to send PACKET_DATA - channel closed", call_id);
                                                return;
                                            }
                                        }

                                        // Send final empty PACKET_DATA to signal end of file
                                        let final_data_packet = Packet {
                                            r#type: PacketType::PacketData as i32,
                                            stat: None,
                                            id: req_id,
                                            data: vec![],
                                        };

                                        if tx.send(Ok(final_data_packet)).await.is_err() {
                                            error!("DiffCopy call_id={} failed to send final PACKET_DATA - channel closed", call_id);
                                            return;
                                        }

                                        debug!(
                                            "DiffCopy call_id={} sent PACKET_DATA for id={} ({})",
                                            call_id,
                                            req_id,
                                            file_path.display()
                                        );
                                    }
                                    Err(e) => {
                                        error!("DiffCopy call_id={} failed to read file id={} ({}): {}", call_id, req_id, file_path.display(), e);
                                        let err_packet = Packet {
                                            r#type: PacketType::PacketErr as i32,
                                            stat: None,
                                            id: req_id,
                                            data: format!("Failed to read file: {}", e)
                                                .into_bytes(),
                                        };
                                        let _ = tx.send(Ok(err_packet)).await;
                                        return;
                                    }
                                }
                            }
                            Some(PacketType::PacketFin) => {
                                // BuildKit signals completion - send PACKET_FIN back and exit
                                debug!("DiffCopy call_id={} received PACKET_FIN, sending PACKET_FIN back", call_id);
                                let fin_packet = Packet {
                                    r#type: PacketType::PacketFin as i32,
                                    stat: None,
                                    id: 0,
                                    data: vec![],
                                };

                                if tx.send(Ok(fin_packet)).await.is_err() {
                                    error!("DiffCopy call_id={} failed to send PACKET_FIN - channel closed", call_id);
                                }

                                debug!("DiffCopy call_id={} transfer complete", call_id);
                                break;
                            }
                            Some(PacketType::PacketErr) => {
                                let err_msg = String::from_utf8_lossy(&packet.data);
                                error!(
                                    "DiffCopy call_id={} received error from BuildKit: {}",
                                    call_id, err_msg
                                );
                                return;
                            }
                            _ => {
                                debug!(
                                    "DiffCopy call_id={} ignoring packet type={:?} id={}",
                                    call_id, packet_type, packet.id
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        debug!("DiffCopy call_id={} incoming stream closed", call_id);
                        break;
                    }
                    Err(e) => {
                        error!("DiffCopy call_id={} stream error: {}", call_id, e);
                        return;
                    }
                }
            }

            // Close the outgoing stream
            drop(tx);
            debug!("DiffCopy call_id={} complete", call_id);
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn tar_stream(
        &self,
        _request: Request<Streaming<Packet>>,
    ) -> Result<Response<Self::DiffCopyStream>, Status> {
        Err(Status::unimplemented(
            "TarStream not implemented - use DiffCopy",
        ))
    }
}

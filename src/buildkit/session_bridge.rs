use anyhow::Result;
use bytes::{BufMut, BytesMut};
use tokio::sync::mpsc;
use tracing::{debug, error};

use super::proto::BytesMessage;

/// gRPC over BytesMessage bridge
///
/// BuildKit's session protocol tunnels gRPC calls through BytesMessage streams.
/// This bridge converts between BytesMessage (raw HTTP/2 frames) and gRPC service calls.
pub struct SessionBridge {
    service_name: String,
}

impl SessionBridge {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Handle incoming BytesMessage stream and route to gRPC services
    pub async fn handle_stream(
        &self,
        mut incoming: tonic::Streaming<BytesMessage>,
        outgoing: mpsc::Sender<BytesMessage>,
    ) -> Result<()> {
        debug!("Session bridge active for service: {}", self.service_name);

        let mut frame_buffer = BytesMut::new();

        while let Ok(Some(msg)) = incoming.message().await {
            // Append raw bytes to frame buffer
            frame_buffer.put_slice(&msg.data);

            // Process complete HTTP/2 frames
            while frame_buffer.len() >= 9 {
                // HTTP/2 frame header is 9 bytes
                let frame_len = u32::from_be_bytes([
                    0,
                    frame_buffer[0],
                    frame_buffer[1],
                    frame_buffer[2],
                ]) as usize;

                if frame_buffer.len() < 9 + frame_len {
                    // Incomplete frame, wait for more data
                    break;
                }

                // Extract complete frame
                let frame_data = frame_buffer.split_to(9 + frame_len);

                // Process frame (for now, just echo it back - proper implementation would route to gRPC service)
                debug!(
                    "Received HTTP/2 frame: {} bytes (type: {})",
                    frame_data.len(),
                    frame_data[3]
                );

                // Send response frame back
                if let Err(e) = outgoing
                    .send(BytesMessage {
                        data: frame_data.to_vec(),
                    })
                    .await
                {
                    error!("Failed to send response frame: {}", e);
                    break;
                }
            }
        }

        debug!("Session bridge closed for service: {}", self.service_name);
        Ok(())
    }
}

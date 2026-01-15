use bytes::{Buf, BytesMut};
use futures_util::task;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_stream::Stream;
use tonic::transport::server::Connected;
use tonic::Streaming;
use tracing::debug;

use super::proto::BytesMessage;

/// Maximum chunk size for BytesMessage (3MB)
/// BuildKit enforces a 3MB limit on BytesMessage chunks
const MAX_CHUNK_SIZE: usize = 3 * 1024 * 1024; // 3MB

/// Stream-to-connection adapter
///
/// Converts a bidirectional gRPC BytesMessage stream into AsyncRead + AsyncWrite.
/// This is the Rust equivalent of BuildKit's streamToConn:
/// https://github.com/moby/buildkit/blob/master/session/grpchijack/dial.go
///
/// BuildKit's session protocol tunnels HTTP/2 over BytesMessage streams.
/// The adapter allows running a gRPC server over the BytesMessage stream.
#[derive(Clone)]
pub struct StreamConn {
    /// Receiver side of the bidirectional stream
    receiver: Arc<Mutex<Streaming<BytesMessage>>>,
    /// Sender side of the bidirectional stream
    sender: Arc<Mutex<mpsc::Sender<BytesMessage>>>,
    /// Buffer for partial reads
    read_buffer: Arc<Mutex<BytesMut>>,
    /// Closed flag
    closed: Arc<Mutex<bool>>,
}

// Implement Unpin (safe since all fields are behind Arc)
impl Unpin for StreamConn {}

impl StreamConn {
    /// Create a new stream-to-connection adapter
    ///
    /// # Arguments
    /// * `receiver` - The incoming BytesMessage stream from Session RPC response
    /// * `sender` - The outgoing BytesMessage sender for Session RPC request
    pub fn new(receiver: Streaming<BytesMessage>, sender: mpsc::Sender<BytesMessage>) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(receiver)),
            sender: Arc::new(Mutex::new(sender)),
            read_buffer: Arc::new(Mutex::new(BytesMut::with_capacity(32 * 1024))), // 32KB buffer
            closed: Arc::new(Mutex::new(false)),
        }
    }

    /// Check if connection is closed
    pub async fn is_closed(&self) -> bool {
        *self.closed.lock().await
    }
}

impl AsyncRead for StreamConn {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Get the receiver
        let receiver_clone = self.receiver.clone();
        let buffer_clone = self.read_buffer.clone();
        let closed_clone = self.closed.clone();

        // Try to lock buffer first
        let mut buffer = match buffer_clone.try_lock() {
            Ok(b) => b,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        // If we have buffered data, return it
        if !buffer.is_empty() {
            let len = std::cmp::min(buffer.len(), buf.remaining());
            buf.put_slice(&buffer[..len]);
            buffer.advance(len);
            return Poll::Ready(Ok(()));
        }

        // Try to receive next message
        let mut receiver = match receiver_clone.try_lock() {
            Ok(r) => r,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        match Pin::new(&mut *receiver).poll_next(cx) {
            Poll::Ready(Some(Ok(msg))) => {
                let preview = if msg.data.len() > 20 {
                    format!("{:?}...", &msg.data[..20])
                } else {
                    format!("{:?}", &msg.data)
                };
                debug!(
                    "StreamConn READ: received {} bytes preview={}",
                    msg.data.len(),
                    preview
                );

                // Put data into buffer
                buffer.clear();
                buffer.extend_from_slice(&msg.data);

                // Copy to output buffer
                let len = std::cmp::min(buffer.len(), buf.remaining());
                buf.put_slice(&buffer[..len]);
                buffer.advance(len);

                debug!("StreamConn READ: provided {} bytes to reader", len);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Some(Err(e))) => {
                debug!("StreamConn receive error: {}", e);
                Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            Poll::Ready(None) => {
                // Stream closed
                debug!("StreamConn closed");
                if let Ok(mut closed) = closed_clone.try_lock() {
                    *closed = true;
                }
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for StreamConn {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let sender_clone = self.sender.clone();

        // Try to lock sender
        let sender = match sender_clone.try_lock() {
            Ok(s) => s,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        // Enforce 3MB chunk size limit
        let chunk_size = std::cmp::min(buf.len(), MAX_CHUNK_SIZE);
        let chunk = &buf[..chunk_size];

        // Create BytesMessage
        let msg = BytesMessage {
            data: chunk.to_vec(),
        };

        // Try to send (non-blocking)
        let msg_clone_for_log = msg.clone();
        match sender.try_send(msg) {
            Ok(_) => {
                let preview = if msg_clone_for_log.data.len() > 20 {
                    format!("{:?}...", &msg_clone_for_log.data[..20])
                } else {
                    format!("{:?}", &msg_clone_for_log.data)
                };
                debug!(
                    "StreamConn WRITE: sent {} bytes preview={}",
                    chunk_size, preview
                );
                Poll::Ready(Ok(chunk_size))
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Channel full, return pending
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "sender closed",
            ))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // No buffering, nothing to flush
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Close the sender
        // Use try_lock() instead of blocking_lock() to avoid blocking the async runtime
        match self.closed.try_lock() {
            Ok(mut closed) => {
                *closed = true;
                Poll::Ready(Ok(()))
            }
            Err(_) => {
                // Lock is held by another task, schedule a wake-up and try again later
                _cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

// Implement Connected trait for tonic
impl Connected for StreamConn {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

// Implement std::io::Read/Write for compatibility
impl std::io::Read for StreamConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Block on async read
        let rt = tokio::runtime::Handle::current();
        let mut read_buf = ReadBuf::new(buf);
        match rt.block_on(async {
            let mut pinned = Pin::new(self);
            let mut cx = Context::from_waker(task::noop_waker_ref());
            pinned.as_mut().poll_read(&mut cx, &mut read_buf)
        }) {
            Poll::Ready(Ok(())) => Ok(read_buf.filled().len()),
            Poll::Ready(Err(e)) => Err(e),
            Poll::Pending => Err(io::Error::new(io::ErrorKind::WouldBlock, "would block")),
        }
    }
}

impl std::io::Write for StreamConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Block on async write
        let rt = tokio::runtime::Handle::current();
        match rt.block_on(async {
            let mut pinned = Pin::new(self);
            let mut cx = Context::from_waker(task::noop_waker_ref());
            pinned.as_mut().poll_write(&mut cx, buf)
        }) {
            Poll::Ready(Ok(n)) => Ok(n),
            Poll::Ready(Err(e)) => Err(e),
            Poll::Pending => Err(io::Error::new(io::ErrorKind::WouldBlock, "would block")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

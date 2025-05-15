// Updated XStream implementation removing check_error_stream and read_rest_after_error

use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

use super::types::{XStreamDirection, XStreamID, XStreamState};

/// XStream struct - represents a pair of streams for data transfer
#[derive(Debug)]
pub struct XStream {
    pub stream_main_read: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
    pub stream_main_write: Arc<tokio::sync::Mutex<futures::io::WriteHalf<Stream>>>,
    pub stream_error_read: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
    pub stream_error_write: Arc<tokio::sync::Mutex<futures::io::WriteHalf<Stream>>>,
    pub id: XStreamID,
    pub peer_id: PeerId,
    // Direction of the stream (inbound or outbound)
    pub direction: XStreamDirection,
    // Closure notifier is now mandatory
    closure_notifier: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    // Stream state (atomic for thread safety)
    state: Arc<AtomicU8>,
}

impl XStream {
    /// Creates a new XStream from components
    pub fn new(
        id: XStreamID,
        peer_id: PeerId,
        stream_main_read: futures::io::ReadHalf<Stream>,
        stream_main_write: futures::io::WriteHalf<Stream>,
        stream_error_read: futures::io::ReadHalf<Stream>,
        stream_error_write: futures::io::WriteHalf<Stream>,
        direction: XStreamDirection,
        closure_notifier: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    ) -> Self {
        info!(
            "Creating new XStream with id: {:?} for peer: {}, direction: {:?}",
            id, peer_id, direction
        );
        Self {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
            stream_error_read: Arc::new(Mutex::new(stream_error_read)),
            stream_error_write: Arc::new(Mutex::new(stream_error_write)),
            id,
            peer_id,
            direction,
            closure_notifier,
            state: Arc::new(AtomicU8::new(XStreamState::Open as u8)),
        }
    }

    /// Get current stream state
    pub fn state(&self) -> XStreamState {
        let value = self.state.load(Ordering::Acquire);
        XStreamState::from(value)
    }

    /// Set the stream state with a specific value
    fn set_state(&self, new_state: XStreamState) {
        let current_state = self.state();

        // Apply state transition rules
        let final_state = match (current_state, new_state) {
            // If already fully closed, stay closed
            (XStreamState::FullyClosed, _) => XStreamState::FullyClosed,
            
            // If write locally closed and read remotely closed, become fully closed
            (XStreamState::WriteLocalClosed, XStreamState::ReadRemoteClosed) => XStreamState::FullyClosed,
            (XStreamState::ReadRemoteClosed, XStreamState::WriteLocalClosed) => XStreamState::FullyClosed,

            // If local closed and remote closes, become fully closed
            (XStreamState::LocalClosed, XStreamState::RemoteClosed) => XStreamState::FullyClosed,

            // If remote closed and local closes, become fully closed
            (XStreamState::RemoteClosed, XStreamState::LocalClosed) => XStreamState::FullyClosed,

            // Otherwise, use the new state
            (_, new_state) => new_state,
        };

        // Update the state
        self.state.store(final_state as u8, Ordering::Release);
        info!(
            "Stream {:?} state changed: {:?} -> {:?}",
            self.id, current_state, final_state
        );
    }

    /// Mark the stream as write locally closed (EOF sent)
    fn mark_write_local_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::WriteLocalClosed),
            XStreamState::ReadRemoteClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already in a more restrictive closed state
        }
    }

    /// Mark the stream as read remotely closed (EOF received)
    fn mark_read_remote_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::ReadRemoteClosed),
            XStreamState::WriteLocalClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already in a more restrictive closed state
        }
    }

    /// Mark the stream as locally closed
    fn mark_local_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::LocalClosed),
            XStreamState::RemoteClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already locally closed or fully closed
        }
    }

    /// Mark the stream as remotely closed
    fn mark_remote_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::RemoteClosed),
            XStreamState::LocalClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already remotely closed or fully closed
        }
    }

    /// Check if the stream is closed (either locally, remotely, or both)
    pub fn is_closed(&self) -> bool {
        match self.state() {
            XStreamState::Open | XStreamState::WriteLocalClosed | XStreamState::ReadRemoteClosed => false,
            _ => true,
        }
    }

    /// Check if the stream is closed locally
    pub fn is_local_closed(&self) -> bool {
        matches!(
            self.state(),
            XStreamState::LocalClosed | XStreamState::FullyClosed
        )
    }

    /// Check if the stream is closed remotely
    pub fn is_remote_closed(&self) -> bool {
        matches!(
            self.state(),
            XStreamState::RemoteClosed | XStreamState::FullyClosed
        )
    }

    /// Check if the stream's write direction is closed locally
    pub fn is_write_local_closed(&self) -> bool {
        matches!(
            self.state(),
            XStreamState::WriteLocalClosed | XStreamState::LocalClosed | XStreamState::FullyClosed
        )
    }

    /// Check if the stream's read direction has received EOF
    pub fn is_read_remote_closed(&self) -> bool {
        matches!(
            self.state(),
            XStreamState::ReadRemoteClosed | XStreamState::RemoteClosed | XStreamState::FullyClosed
        )
    }

    /// Helper method to send closure notification when EOF or connection error is detected
    fn notify_eof(&self, reason: &str) {
        // Mark as remotely closed first
        return;
        self.mark_read_remote_closed();

        info!(
            "Sending closure notification for stream {:?} due to: {}",
            self.id, reason
        );
        match self.closure_notifier.send((self.peer_id, self.id)) {
            Ok(_) => info!(
                "Closure notification sent successfully for stream {:?}",
                self.id
            ),
            Err(e) => warn!(
                "Failed to send closure notification for stream {:?}: {}",
                self.id, e
            ),
        }
    }

    /// Validate that the stream is in a valid state for reading
    fn check_readable(&self) -> Result<(), std::io::Error> {
        return Ok(());
        if self.is_read_remote_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("Cannot read from stream {:?}: EOF received", self.id),
            ));
        }
        if self.is_remote_closed() || self.is_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Cannot read from stream {:?}: stream closed", self.id),
            ));
        }
        Ok(())
    }

    /// Validate that the stream is in a valid state for writing
    fn check_writable(&self) -> Result<(), std::io::Error> {
        return Ok(());
        if self.is_write_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Cannot write to stream {:?}: write half closed", self.id),
            ));
        }
        if self.is_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Cannot write to stream {:?}: locally closed", self.id),
            ));
        }
        if self.is_remote_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Cannot write to stream {:?}: remotely closed", self.id),
            ));
        }
        Ok(())
    }

    /// Checks if an error indicates that the connection was closed by remote
    fn is_connection_closed_error(&self, e: &std::io::Error) -> bool {
        matches!(
            e.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
        )
    }

    /// Reads exact number of bytes from the main stream
    pub async fn read_exact(&self, size: usize) -> Result<Vec<u8>, std::io::Error> {
        // First check if we can read
        self.check_readable()?;

        let mut buf = vec![0u8; size];
        let stream_main_read = self.stream_main_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_main_read.lock().await;
            guard.read_exact(&mut buf).await
        };

        // Process the result
        match read_result {
            Ok(_) => Ok(buf),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof
                    || self.is_connection_closed_error(&e)
                {
                    info!("Detected EOF/connection error while reading exact bytes from stream {:?}: {:?}", self.id, e.kind());
                    self.mark_read_remote_closed();
                    self.notify_eof(&format!("read_exact error: {:?}", e.kind()));
                }
                Err(e)
            }
        }
    }

    /// Reads all data from the main stream to the end
    pub async fn read_to_end(&self) -> Result<Vec<u8>, std::io::Error> {
        // First check if we can read
        self.check_readable()?;

        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_main_read.lock().await;
            guard.read_to_end(&mut buf).await
        };

        match read_result {
            Ok(bytes_read) => {
                info!("Read {} bytes to end from stream {:?}", bytes_read, self.id);

                // If we read zero bytes and this is the first read, it might be an EOF
                if bytes_read == 0 && buf.is_empty() {
                    info!("Stream {:?} was already at EOF", self.id);
                    self.mark_read_remote_closed();
                    self.notify_eof("read_to_end returned 0 bytes");
                }
                self.notify_eof("read_to_end all");
                Ok(buf)
            }
            Err(e) => {
                if self.is_connection_closed_error(&e) {
                    info!(
                        "Detected connection error during read_to_end for stream {:?}: {:?}",
                        self.id,
                        e.kind()
                    );
                    self.mark_read_remote_closed();
                    self.notify_eof(&format!("read_to_end error: {:?}", e.kind()));
                }
                Err(e)
            }
        }
    }

    /// Reads available data from the main stream
    pub async fn read(&self) -> Result<Vec<u8>, std::io::Error> {
        // First check if we can read
        self.check_readable()?;

        let mut buf: Vec<u8> = vec![0; 4096]; // Use a reasonable buffer size
        let stream_main_read = self.stream_main_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_main_read.lock().await;
            guard.read(&mut buf).await
        };

        match read_result {
            Ok(bytes_read) => {
                // Check for EOF condition (remote side closed the stream)
                if bytes_read == 0 {
                    info!("Detected EOF while reading from stream {:?}", self.id);
                    self.mark_read_remote_closed();
                    self.notify_eof("read returned 0 bytes");

                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "End of file",
                    ));
                }

                // Resize the buffer to the actual bytes read
                buf.truncate(bytes_read);
                Ok(buf)
            }
            Err(e) => {
                if self.is_connection_closed_error(&e) {
                    info!(
                        "Detected connection error during read for stream {:?}: {:?}",
                        self.id,
                        e.kind()
                    );
                    self.mark_read_remote_closed();
                    self.notify_eof(&format!("read error: {:?}", e.kind()));
                }
                Err(e)
            }
        }
    }

    /// Writes all data to the main stream
    pub async fn write_all(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        // First check if we can write
        self.check_writable()?;

        let stream_main_write = self.stream_main_write.clone();

        // Acquire lock, write data, and flush
        let mut guard = stream_main_write.lock().await;

        // Try to write
        match guard.write_all(&buf).await {
            Ok(_) => {
                // Write succeeded, try to flush
                match guard.flush().await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        // Check if the error indicates a broken pipe or connection reset
                        if self.is_connection_closed_error(&e) {
                            info!(
                                "Detected remote closure during flush for stream {:?}",
                                self.id
                            );
                            println!("111111111111111 2");
                            self.mark_remote_closed();
                            self.notify_eof(&format!("flush error: {:?}", e.kind()));
                        }
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // Check if the error indicates a broken pipe or connection reset
                if self.is_connection_closed_error(&e) {
                    info!(
                        "Detected remote closure during write for stream {:?}",
                        self.id
                    );
                    println!("111111111111111 3");
                    self.mark_remote_closed();
                    self.notify_eof(&format!("write error: {:?}", e.kind()));
                }
                Err(e)
            }
        }
    }

    /// Closes only the write half of the main stream, sending EOF
    /// This allows the peer to know all data has been sent
    /// while still allowing us to read their response
    pub async fn write_eof(&self) -> Result<(), std::io::Error> {
        // Check if the write half is already closed
        if self.is_write_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Cannot write EOF to stream {:?}: write half already closed", self.id),
            ));
        }

        let stream_main_write = self.stream_main_write.clone();

        // Acquire lock on the write half
        let mut guard = stream_main_write.lock().await;

        // Flush any pending data first
        if let Err(e) = guard.flush().await {
            if self.is_connection_closed_error(&e) {
                info!("Detected remote closure during flush for stream {:?}", self.id);
                println!("111111111111111 4");
                self.mark_remote_closed();
                self.notify_eof(&format!("flush error: {:?}", e.kind()));
            }
            return Err(e);
        }

        // Shutdown only the write half (this is different from close())
        match guard.close().await {
            Ok(_) => {
                info!("Stream {:?} write half shutdown (EOF sent)", self.id);
                // Mark the write half as closed but keep read half open
                self.mark_write_local_closed();
                Ok(())
            }
            Err(e) => {
                if self.is_connection_closed_error(&e) {
                    info!("Detected remote closure during shutdown for stream {:?}", self.id);
                    println!("111111111111111 5");
                    self.mark_remote_closed();
                    self.notify_eof(&format!("shutdown error: {:?}", e.kind()));
                    // Remote already closed, consider EOF sent
                    self.mark_write_local_closed();
                    return Ok(());
                }
                Err(e)
            }
        }
    }

    /// Read from the error stream
    pub async fn error_read(&self) -> Result<Vec<u8>, std::io::Error> {
        // Only outbound streams should read from error stream
        if self.direction != XStreamDirection::Outbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only outbound streams can read from error stream",
            ));
        }

        let mut buf: Vec<u8> = Vec::new();
        let stream_error_read = self.stream_error_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_error_read.lock().await;
            guard.read_to_end(&mut buf).await
        };

        match read_result {
            Ok(_) => Ok(buf),
            Err(e) => Err(e),
        }
    }

    /// Write an error to the error stream (only for inbound streams)
    pub async fn error_write(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        // Only inbound streams can write errors
        if self.direction != XStreamDirection::Inbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only inbound streams can write to error stream",
            ));
        }
        // Get the error stream write handle
        let stream_error_write = self.stream_error_write.clone();
        let mut error_stream = stream_error_write.lock().await;

        // Write the error message
        match error_stream.write_all(&buf).await {
            Ok(_) => match error_stream.flush().await {
                Ok(_) => {
                    info!("Error written to stream {:?}", self.id);
                }
                Err(e) => {
                    error!("Failed to flush error stream {:?}: {}", self.id, e);
                    return Err(e)
                }
            },
            Err(e) => {
                error!("Failed to write to error stream {:?}: {}", self.id, e);
                return Err(e)
            }
        }

        match error_stream.close().await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to write to error stream {:?}: {}", self.id, e);
                Err(e)
            }
        }
    }

    /// Write a string error to the error stream (helper method)
    pub async fn write_error(&self, error_message: &str) -> Result<(), std::io::Error> {
        self.error_write(error_message.as_bytes().to_vec()).await
    }

    /// Closes the streams
    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        info!(
            "Closing XStream with id: {:?} for peer: {}",
            self.id, self.peer_id
        );

        // If already closed locally, return early
        if self.is_local_closed() {
            debug!("Stream {:?} already locally closed", self.id);
            return Ok(());
        }

        // For inbound streams, we need to send a no-error marker before closing
        if self.direction == XStreamDirection::Inbound {
            // Close the error stream
            let stream_error_write = self.stream_error_write.clone();
            let mut error_stream = stream_error_write.lock().await;
            let _ = error_stream.close().await;
        }

        // Mark as locally closed
        self.mark_local_closed();

        // Get a lock on the write stream and close it
        let stream_main_write = self.stream_main_write.clone();

        // Try to flush and close, catching potential errors that indicate closed connection
        let result = {
            let mut guard = stream_main_write.lock().await;

            // Try to flush
            match guard.flush().await {
                Ok(_) => {
                    // Flush worked, try to close
                    match guard.close().await {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            // Check if error indicates the stream was already closed by remote
                            if self.is_connection_closed_error(&e) {
                                info!("Remote already closed connection during close() for stream {:?}", self.id);
                                println!("111111111111111 6");
                                self.mark_remote_closed();
                                // Return success if remote already closed it
                                Ok(())
                            } else {
                                Err(e)
                            }
                        }
                    }
                }
                Err(e) => {
                    // Check if error indicates the stream was already closed by remote
                    if self.is_connection_closed_error(&e) {
                        info!(
                            "Remote already closed connection during flush for stream {:?}",
                            self.id
                        );
                        println!("111111111111111 7");
                        self.mark_remote_closed();
                        // Return success if remote already closed it
                        Ok(())
                    } else {
                        Err(e)
                    }
                }
            }
        };

        debug!("Network stream close result: {:?}", result);

        // If we detected a connection error, mark it as remotely closed
        if let Err(ref e) = result {
            if self.is_connection_closed_error(e) {
                self.mark_remote_closed();
            }
        }
        return result;

        // Send closure notification
        debug!(
            "Sending closure notification for stream {:?} of peer {}",
            self.id, self.peer_id
        );

        // This is non-blocking and returns immediately
        match self.closure_notifier.send((self.peer_id, self.id)) {
            Ok(_) => debug!(
                "Close notification sent successfully for stream {:?}",
                self.id
            ),
            Err(e) => warn!(
                "Failed to send close notification for stream {:?}: {}",
                self.id, e
            ),
        }

        // Add a debug print just before returning
        debug!(
            "Stream close complete for {:?} - state: {:?}, result: {:?}",
            self.id,
            self.state(),
            result
        );
        result
    }
}

impl Clone for XStream {
    fn clone(&self) -> Self {
        debug!(
            "Cloning XStream with id: {:?} for peer: {}",
            self.id, self.peer_id
        );

        // Clone the stream with all its components
        Self {
            stream_main_read: self.stream_main_read.clone(),
            stream_main_write: self.stream_main_write.clone(),
            stream_error_read: self.stream_error_read.clone(),
            stream_error_write: self.stream_error_write.clone(),
            id: self.id,
            peer_id: self.peer_id,
            direction: self.direction,
            closure_notifier: self.closure_notifier.clone(),
            state: self.state.clone(),
        }
    }
}
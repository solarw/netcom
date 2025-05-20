// Updated XStream implementation using the new state management module

use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

use super::types::{XStreamDirection, XStreamID, XStreamState};
use super::xstream_state::XStreamStateManager;

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
    // State manager handling all state transitions and notifications
    state_manager: XStreamStateManager,
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
        
        // Create the state manager
        let state_manager = XStreamStateManager::new(
            id,
            peer_id,
            direction,
            closure_notifier,
        );
        
        Self {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
            stream_error_read: Arc::new(Mutex::new(stream_error_read)),
            stream_error_write: Arc::new(Mutex::new(stream_error_write)),
            id,
            peer_id,
            direction,
            state_manager,
        }
    }

    /// Get current stream state
    pub fn state(&self) -> XStreamState {
        self.state_manager.state()
    }

    /// Check if the stream is closed (either locally, remotely, or both)
    pub fn is_closed(&self) -> bool {
        self.state_manager.is_closed()
    }

    /// Check if the stream is closed locally
    pub fn is_local_closed(&self) -> bool {
        self.state_manager.is_local_closed()
    }

    /// Check if the stream is closed remotely
    pub fn is_remote_closed(&self) -> bool {
        self.state_manager.is_remote_closed()
    }

    /// Check if the stream's write direction is closed locally
    pub fn is_write_local_closed(&self) -> bool {
        self.state_manager.is_write_local_closed()
    }

    /// Check if the stream's read direction has received EOF
    pub fn is_read_remote_closed(&self) -> bool {
        self.state_manager.is_read_remote_closed()
    }

    /// Checks if the stream is in a valid state for reading
    fn check_readable(&self) -> Result<(), std::io::Error> {
        if self.state_manager.is_read_remote_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("Cannot read from stream {:?}: EOF received", self.id),
            ));
        }
        if self.state_manager.is_remote_closed() || self.state_manager.is_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Cannot read from stream {:?}: stream closed", self.id),
            ));
        }
        Ok(())
    }

    /// Checks if the stream is in a valid state for writing
    fn check_writable(&self) -> Result<(), std::io::Error> {
        if self.state_manager.is_write_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Cannot write to stream {:?}: write half closed", self.id),
            ));
        }
        if self.state_manager.is_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Cannot write to stream {:?}: locally closed", self.id),
            ));
        }
        if self.state_manager.is_remote_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Cannot write to stream {:?}: remotely closed", self.id),
            ));
        }
        Ok(())
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
                if e.kind() == std::io::ErrorKind::UnexpectedEof || 
                   self.state_manager.handle_connection_error(&e, "read_exact error") {
                    self.state_manager.mark_read_remote_closed();
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
                debug!("Read {} bytes to end from stream {:?}", bytes_read, self.id);

                // If we read zero bytes and this is the first read, it might be an EOF
                if bytes_read == 0 && buf.is_empty() {
                    debug!("Stream {:?} was already at EOF", self.id);
                    self.state_manager.mark_read_remote_closed();
                }
                Ok(buf)
            }
            Err(e) => {
                self.state_manager.handle_connection_error(&e, "read_to_end error");
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
                    debug!("Detected EOF while reading from stream {:?}", self.id);
                    self.state_manager.mark_read_remote_closed();

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
                self.state_manager.handle_connection_error(&e, "read error");
                Err(e)
            }
        }
    }

    /// Read data after an error has been received or written
    ///
    /// This method allows reading the remaining data from the main stream
    /// after an error has been received through `error_read()` or sent via `write_error()`.
    /// Unlike normal read methods, this won't return an error if the stream is in error state.
    pub async fn read_after_error(&self) -> Result<Vec<u8>, std::io::Error> {
        // Ensure proper context: either we've read an error, or we've written one
        let has_error = self.state_manager.has_error_written() || 
                       self.state_manager.has_error_data().await;
                        
        if !has_error {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot use read_after_error() without first receiving or writing an error",
            ));
        }
        
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_main_read.lock().await;
            guard.read_to_end(&mut buf).await
        };

        match read_result {
            Ok(_) => {
                debug!("Read {} bytes after error from stream {:?}", buf.len(), self.id);
                Ok(buf)
            }
            Err(e) => {
                // If it's an EOF or connection closed error, return empty buffer
                if e.kind() == std::io::ErrorKind::UnexpectedEof || 
                   self.state_manager.is_connection_closed_error(&e) {
                    debug!("EOF or connection closed when reading after error from stream {:?}", self.id);
                    Ok(Vec::new())
                } else {
                    // For other errors, return the actual error
                    error!("Error reading after error from stream {:?}: {}", self.id, e);
                    Err(e)
                }
            }
        }
    }

    /// Closes only the write half of the main stream, sending EOF
    /// This allows the peer to know all data has been sent
    /// while still allowing us to read their response
    pub async fn write_eof(&self) -> Result<(), std::io::Error> {
        // Check if the write half is already closed
        if self.state_manager.is_write_local_closed() {
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
            if self.state_manager.handle_connection_error(&e, "flush error during write_eof") {
                return Err(e);
            }
        }

        // Shutdown only the write half (this is different from close())
        match guard.close().await {
            Ok(_) => {
                debug!("Stream {:?} write half shutdown (EOF sent)", self.id);
                // Mark the write half as closed but keep read half open
                self.state_manager.mark_write_local_closed();
                Ok(())
            }
            Err(e) => {
                // If the remote has already closed, consider it a success
                if self.state_manager.handle_connection_error(&e, "shutdown error during write_eof") {
                    self.state_manager.mark_write_local_closed();
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Read from the error stream (only for outbound streams)
    pub async fn error_read(&self) -> Result<Vec<u8>, std::io::Error> {
        // Only outbound streams should read from error stream
        if self.direction != XStreamDirection::Outbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only outbound streams can read from error stream",
            ));
        }

        // Check if we already have stored error data
        if let Some(data) = self.state_manager.get_error_data().await {
            debug!("Returning cached error data for stream {:?}", self.id);
            return Ok(data);
        }

        // If no stored data, read from the error stream
        let mut buf: Vec<u8> = Vec::new();
        let stream_error_read = self.stream_error_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_error_read.lock().await;
            guard.read_to_end(&mut buf).await
        };

        match read_result {
            Ok(_) => {
                // Store the error data for future reads
                if !buf.is_empty() {
                    self.state_manager.store_error_data(buf.clone()).await;
                }
                Ok(buf)
            },
            Err(e) => Err(e),
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
                        self.state_manager.handle_connection_error(&e, "flush error");
                        Err(e)
                    }
                }
            }
            Err(e) => {
                self.state_manager.handle_connection_error(&e, "write error");
                Err(e)
            }
        }
    }

    /// Closes the streams
    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        info!(
            "Closing XStream with id: {:?} for peer: {}",
            self.id, self.peer_id
        );

        // If already closed locally, return early
        if self.state_manager.is_local_closed() {
            debug!("Stream {:?} already locally closed", self.id);
            return Ok(());
        }

        // For inbound streams, close the error stream
        if self.direction == XStreamDirection::Inbound {
            let stream_error_write = self.stream_error_write.clone();
            let mut error_stream = stream_error_write.lock().await;
            let _ = error_stream.close().await;
        }

        // Mark as locally closed
        self.state_manager.mark_local_closed();

        // Get a lock on the write stream and close it
        let stream_main_write = self.stream_main_write.clone();

        // Try to flush and close, catching potential errors that indicate closed connection
        let result = {
            let mut guard = stream_main_write.lock().await;

            // Try to flush
            match guard.flush().await {
                Ok(_) => {
                    // Flush worked, try to close
                    guard.close().await
                }
                Err(e) => {
                    // Check if error indicates the stream was already closed by remote
                    if self.state_manager.handle_connection_error(&e, "flush error during close") {
                        // Return success if remote already closed it
                        Ok(())
                    } else {
                        Err(e)
                    }
                }
            }
        };

        debug!("Network stream close result: {:?}", result);

        // Return the result of closing the stream
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
            state_manager: self.state_manager.clone(),
        }
    }
}

impl Drop for XStream {
    fn drop(&mut self) {
        debug!("Dropping XStream with id: {:?}", self.id);
        
        // If stream is not fully closed, notify about drop
        if !self.state_manager.is_closed() {
            self.state_manager.notify_state_change("XStream dropped");
        }
    }
}
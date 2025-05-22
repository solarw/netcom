// xstream.rs
// Updated XStream implementation using the new error handling module
// With utility methods to reduce code duplication

use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

use super::types::{XStreamDirection, XStreamID, XStreamState};
use super::xstream_state::XStreamStateManager;
use super::error_handling::{ErrorDataStore, ErrorReaderTask};

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
    
    // New error handling components
    error_data_store: ErrorDataStore,
    error_reader_task: Arc<Mutex<Option<ErrorReaderTask>>>,
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
        let state_manager = XStreamStateManager::new(id, peer_id, direction, closure_notifier.clone());
        let error_data_store = ErrorDataStore::new();
        
        // Create Arc-wrapped streams first
        let stream_error_read_arc = Arc::new(Mutex::new(stream_error_read));
        let stream_error_write_arc = Arc::new(Mutex::new(stream_error_write));
        
        // Start error reading task for outbound streams
        let error_reader_task = if direction == XStreamDirection::Outbound {
            let task = ErrorReaderTask::start(
                id,
                peer_id,
                direction,
                stream_error_read_arc.clone(),
                error_data_store.clone(),
                closure_notifier,
            );
            Arc::new(Mutex::new(Some(task)))
        } else {
            Arc::new(Mutex::new(None))
        };

        Self {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
            stream_error_read: stream_error_read_arc,
            stream_error_write: stream_error_write_arc,
            id,
            peer_id,
            direction,
            state_manager,
            error_data_store,
            error_reader_task,
        }
    }

    // ===== UTILITY METHODS TO REDUCE CODE DUPLICATION =====

    /// Executes a read operation on the main stream with proper error handling
    async fn execute_main_read_op<F, R>(&self, operation: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(
            &mut futures::io::ReadHalf<Stream>,
        ) -> futures::future::BoxFuture<'_, Result<R, std::io::Error>>,
    {
        // First check if we can read
        self.check_readable()?;

        let stream_main_read = self.stream_main_read.clone();

        // Acquire the lock and perform the read operation
        let read_result = {
            let mut guard = stream_main_read.lock().await;
            operation(&mut *guard).await
        };

        match read_result {
            Ok(result) => Ok(result),
            Err(e) => {
                // Handle EOF and connection errors
                if e.kind() == std::io::ErrorKind::UnexpectedEof
                    || self
                        .state_manager
                        .handle_connection_error(&e, "read operation error")
                {
                    self.state_manager.mark_read_remote_closed();
                }
                Err(e)
            }
        }
    }

    /// Executes a write operation on the main stream with proper error handling
    async fn execute_main_write_op<F, R>(&self, operation: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(
            &mut futures::io::WriteHalf<Stream>,
        ) -> futures::future::BoxFuture<'_, Result<R, std::io::Error>>,
    {
        // First check if we can write
        self.check_writable()?;

        let stream_main_write = self.stream_main_write.clone();

        // Acquire the lock and perform the write operation
        let write_result = {
            let mut guard = stream_main_write.lock().await;
            operation(&mut *guard).await
        };

        match write_result {
            Ok(result) => Ok(result),
            Err(e) => {
                // Handle connection errors
                self.state_manager
                    .handle_connection_error(&e, "write operation error");
                Err(e)
            }
        }
    }

    /// Executes a read operation on the error stream with proper error handling
    async fn execute_error_read_op<F, R>(&self, operation: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(
            &mut futures::io::ReadHalf<Stream>,
        ) -> futures::future::BoxFuture<'_, Result<R, std::io::Error>>,
    {
        let stream_error_read = self.stream_error_read.clone();

        // Acquire the lock and perform the operation
        let op_result = {
            let mut guard = stream_error_read.lock().await;
            operation(&mut *guard).await
        };

        match op_result {
            Ok(result) => Ok(result),
            Err(e) => {
                self.state_manager
                    .handle_connection_error(&e, "error stream read operation error");
                Err(e)
            }
        }
    }

    /// Executes a write operation on the error stream with proper error handling
    async fn execute_error_write_op<F, R>(&self, operation: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(
            &mut futures::io::WriteHalf<Stream>,
        ) -> futures::future::BoxFuture<'_, Result<R, std::io::Error>>,
    {
        let stream_error_write = self.stream_error_write.clone();

        // Acquire the lock and perform the operation
        let op_result = {
            let mut guard = stream_error_write.lock().await;
            operation(&mut *guard).await
        };

        match op_result {
            Ok(result) => Ok(result),
            Err(e) => {
                self.state_manager
                    .handle_connection_error(&e, "error stream write operation error");
                Err(e)
            }
        }
    }

    // ===== STATE MANAGEMENT METHODS =====

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

    // ===== STREAM OPERATIONS =====

    /// Reads exact number of bytes from the main stream
    pub async fn read_exact(&self, size: usize) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = vec![0u8; size];

        self.execute_main_read_op(|reader| {
            Box::pin(async move {
                reader.read_exact(&mut buf).await?;
                Ok(buf)
            })
        })
        .await
    }

    /// Reads all data from the main stream to the end
    pub async fn read_to_end(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();

        self.execute_main_read_op(|reader| {
            Box::pin(async move {
                let bytes_read = reader.read_to_end(&mut buf).await?;

                // If we read zero bytes and this is the first read, it might be an EOF
                if bytes_read == 0 && buf.is_empty() {
                    debug!("Stream was already at EOF");
                }

                Ok(buf)
            })
        })
        .await
    }

    /// Reads available data from the main stream
    pub async fn read(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = vec![0; 4096]; // Use a reasonable buffer size

        self.execute_main_read_op(|reader| {
            Box::pin(async move {
                let bytes_read = reader.read(&mut buf).await?;

                // Check for EOF condition (remote side closed the stream)
                if bytes_read == 0 {
                    debug!("Detected EOF while reading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "End of file",
                    ));
                }

                // Resize the buffer to the actual bytes read
                buf.truncate(bytes_read);
                Ok(buf)
            })
        })
        .await
    }

    /// Writes all data to the main stream
    pub async fn write_all(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        self.execute_main_write_op(|writer| {
            let data = buf.clone(); // Clone for move into async block
            Box::pin(async move {
                writer.write_all(&data).await?;
                Ok(())
            })
        })
        .await
    }

    /// Flushes the main stream
    pub async fn flush(&self) -> Result<(), std::io::Error> {
        self.execute_main_write_op(|writer| Box::pin(async move { writer.flush().await }))
            .await
    }

    /// Closes only the write half of the main stream, sending EOF
    /// This allows the peer to know all data has been sent
    /// while still allowing us to read their response
    pub async fn write_eof(&self) -> Result<(), std::io::Error> {
        // Check if the write half is already closed
        if self.state_manager.is_write_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!(
                    "Cannot write EOF to stream {:?}: write half already closed",
                    self.id
                ),
            ));
        }

        let result = self
            .execute_main_write_op(|writer| {
                Box::pin(async move {
                    // Flush any pending data first
                    writer.flush().await?;

                    // Shutdown only the write half (this is different from close())
                    writer.close().await?;

                    Ok(())
                })
            })
            .await;

        // Mark state change on success or connection errors
        match result {
            Ok(_) => {
                debug!("Stream {:?} write half shutdown (EOF sent)", self.id);
                self.state_manager.mark_write_local_closed();
                Ok(())
            }
            Err(e) => {
                // If the remote has already closed, consider it a success
                if self
                    .state_manager
                    .handle_connection_error(&e, "shutdown error during write_eof")
                {
                    self.state_manager.mark_write_local_closed();
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    // ===== ERROR STREAM OPERATIONS =====

    /// Read from the error stream (only for outbound streams)
    /// This method keeps the same signature but now waits for error from the background task
    pub async fn error_read(&self) -> Result<Vec<u8>, std::io::Error> {
        // Only outbound streams should read from error stream
        if self.direction != XStreamDirection::Outbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only outbound streams can read from error stream",
            ));
        }

        debug!("Waiting for error data for stream {:?}", self.id);
        
        // Wait for error data from the background task
        self.error_data_store.wait_for_error().await
    }

    /// Internal method to read from error stream (used by background task)
    /// This is the actual reading functionality that was moved from error_read
    pub async fn inner_error_read(&self) -> Result<Vec<u8>, std::io::Error> {
        // Only outbound streams should read from error stream
        if self.direction != XStreamDirection::Outbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only outbound streams can read from error stream",
            ));
        }

        // Read from the error stream using execute_error_read_op
        let mut buf: Vec<u8> = Vec::new();

        let read_result = self
            .execute_error_read_op(|reader| {
                Box::pin(async move {
                    reader.read_to_end(&mut buf).await?;
                    Ok(buf)
                })
            })
            .await;

        match read_result {
            Ok(buf) => {
                debug!("Read {} bytes from error stream for stream {:?}", buf.len(), self.id);
                Ok(buf)
            }
            Err(e) => {
                error!("Failed to read from error stream for stream {:?}: {:?}", self.id, e);
                Err(e)
            }
        }
    }

    /// Check if error data is available without waiting
    pub async fn has_error_data(&self) -> bool {
        self.error_data_store.has_error().await
    }

    /// Get cached error data if available (non-blocking)
    pub async fn get_cached_error(&self) -> Option<Vec<u8>> {
        self.error_data_store.get_cached_error().await
    }

    /// Write to the error stream (only for inbound streams)
    pub async fn error_write(&self, error_data: Vec<u8>) -> Result<(), std::io::Error> {
        // Only inbound streams should write to error stream
        if self.direction != XStreamDirection::Inbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only inbound streams can write to error stream",
            ));
        }

        // Check if we've already written an error
        if self.state_manager.has_error_written() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Error already written to this stream",
            ));
        }

        // Mark that we're writing an error
        self.state_manager.mark_error_written();

        // Write the error data to the error stream using execute_error_write_op
        let data_clone = error_data.clone();
        let result = self
            .execute_error_write_op(|writer| {
                let error_data = data_clone.clone();
                Box::pin(async move {
                    writer.write_all(&error_data).await?;
                    writer.flush().await?;
                    writer.close().await?;
                    Ok(())
                })
            })
            .await;

        match result {
            Ok(_) => {
                // Mark stream state as error
                self.state_manager
                    .mark_error("Error written to error stream");
                debug!("Error successfully written to stream {:?}", self.id);
                Ok(())
            }
            Err(e) => {
                self.state_manager
                    .handle_connection_error(&e, "write error during write_error");
                Err(e)
            }
        }
    }

    /// Closes the streams and shuts down background tasks
    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        info!(
            "Closing XStream with id: {:?} for peer: {}",
            self.id, self.peer_id
        );

        // Always mark as locally closed first
        self.state_manager.mark_local_closed();
        debug!("Stream {:?} marked as locally closed", self.id);

        // Shutdown error reader task
        {
            let mut task_guard = self.error_reader_task.lock().await;
            if let Some(task) = task_guard.take() {
                debug!("Shutting down error reader task for stream {:?}", self.id);
                task.shutdown().await;
            }
        }

        // Close error data store
        self.error_data_store.close().await;

        // If already fully closed, return early
        if self.state_manager.is_closed() {
            debug!("Stream {:?} already fully closed", self.id);
            return Ok(());
        }

        // For inbound streams, close the error stream
        if self.direction == XStreamDirection::Inbound {
            let _ = self
                .execute_error_write_op(|writer| Box::pin(async move { writer.close().await }))
                .await;
        }

        // Close main write stream
        let result = self
            .execute_main_write_op(|writer| {
                Box::pin(async move {
                    let _ = writer.flush().await;
                    writer.close().await
                })
            })
            .await;

        debug!("Network stream close result: {:?}", result);

        // Notify about the state change
        self.state_manager
            .notify_state_change("Stream explicitly closed");

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if self.state_manager.is_connection_closed_error(&e) {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl Clone for XStream {
    fn clone(&self) -> Self {
        debug!(
            "Cloning XStream with id: {:?} for peer: {}",
            self.id, self.peer_id
        );

        Self {
            stream_main_read: self.stream_main_read.clone(),
            stream_main_write: self.stream_main_write.clone(),
            stream_error_read: self.stream_error_read.clone(),
            stream_error_write: self.stream_error_write.clone(),
            id: self.id,
            peer_id: self.peer_id,
            direction: self.direction,
            state_manager: self.state_manager.clone(),
            error_data_store: self.error_data_store.clone(),
            error_reader_task: self.error_reader_task.clone(),
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

        // The error reader task will be shut down when the Arc is dropped
        // or when close() is called explicitly
    }
}
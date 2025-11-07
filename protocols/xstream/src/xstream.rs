// xstream.rs
// Updated XStream implementation with enhanced error handling
// With utility methods to reduce code duplication

use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::select;
use tracing::{debug, error, info, warn};

use super::types::{XStreamDirection, XStreamID, XStreamState};
use super::xstream_state::XStreamStateManager;
use super::error_handling::{ErrorDataStore, ErrorReaderTask};
use super::xstream_error::{ErrorOnRead, ReadError, XStreamError, XStreamReadResult, utils};

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
    
    // Error handling components
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
        self.check_readable_basic()?;

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

    /// Basic readable check for internal operations (returns std::io::Error)
    fn check_readable_basic(&self) -> Result<(), std::io::Error> {
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

    /// Enhanced readable check that returns ErrorOnRead
    fn check_readable(&self) -> XStreamReadResult<()> {
        match self.check_readable_basic() {
            Ok(()) => Ok(()),
            Err(e) => Err(ErrorOnRead::io_error_only(e)),
        }
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

    /// Check if there's an immediate error available
    async fn check_for_immediate_error(&self) -> Option<XStreamError> {
        if self.direction == XStreamDirection::Outbound {
            if let Some(error_data) = self.error_data_store.get_cached_error().await {
                return Some(XStreamError::new(error_data));
            }
        }
        None
    }

    /// Check if there's a pending error without blocking
    pub async fn has_pending_error(&self) -> bool {
        self.direction == XStreamDirection::Outbound && self.error_data_store.has_error().await
    }

    // ===== ENHANCED STREAM OPERATIONS WITH ERROR HANDLING =====

    /// Reads exact number of bytes from the main stream with error awareness
    pub async fn read_exact(&self, size: usize) -> XStreamReadResult<Vec<u8>> {
        // Check stream state first
        self.check_readable()?;

        // Check for immediate error
        if let Some(error) = self.check_for_immediate_error().await {
            return Err(ErrorOnRead::xstream_error_only(error));
        }

        // For outbound streams, read with error awareness
        if self.direction == XStreamDirection::Outbound {
            self.read_exact_with_error_awareness(size).await
        } else {
            // For inbound streams, simple read
            self.read_exact_simple(size).await
        }
    }

    /// Simple read_exact for inbound streams
    async fn read_exact_simple(&self, size: usize) -> XStreamReadResult<Vec<u8>> {
        let mut buf = vec![0u8; size];

        match self.execute_main_read_op(|reader| {
            Box::pin(async move {
                reader.read_exact(&mut buf).await?;
                Ok(buf)
            })
        }).await {
            Ok(data) => Ok(data),
            Err(e) => Err(ErrorOnRead::io_error_only(e)),
        }
    }

    /// Read exact with error awareness for outbound streams
    async fn read_exact_with_error_awareness(&self, size: usize) -> XStreamReadResult<Vec<u8>> {
        let mut buf = vec![0u8; size];
        let mut bytes_read = 0;

        while bytes_read < size {
            let stream_main_read = self.stream_main_read.clone();
            
            select! {
                // Try to read more data
                read_result = async {
                    let mut guard = stream_main_read.lock().await;
                    guard.read(&mut buf[bytes_read..]).await
                } => {
                    match read_result {
                        Ok(0) => {
                            // EOF reached before reading all data
                            let partial_data = buf[0..bytes_read].to_vec();
                            let eof_error = std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                format!("EOF after reading {} of {} bytes", bytes_read, size)
                            );
                            return Err(ErrorOnRead::from_io_error(partial_data, eof_error));
                        },
                        Ok(n) => {
                            bytes_read += n;
                            debug!("Read {} bytes, total: {}/{}", n, bytes_read, size);
                        },
                        Err(e) => {
                            // IO error during read
                            let partial_data = buf[0..bytes_read].to_vec();
                            self.state_manager.handle_connection_error(&e, "read_exact error");
                            return Err(ErrorOnRead::from_io_error(partial_data, e));
                        }
                    }
                },
                // Wait for error from server
                error_result = self.error_data_store.wait_for_error() => {
                    match error_result {
                        Ok(error_data) => {
                            // Server sent an error
                            let partial_data = buf[0..bytes_read].to_vec();
                            let xstream_error = XStreamError::new(error_data);
                            return Err(ErrorOnRead::from_xstream_error(partial_data, xstream_error));
                        },
                        Err(_) => {
                            // Error stream closed, continue reading
                            debug!("Error stream closed, continuing to read data");
                        }
                    }
                }
            }
        }

        // Successfully read all requested bytes
        buf.truncate(size);
        Ok(buf)
    }

    /// Reads all data from the main stream to the end with error awareness
    pub async fn read_to_end(&self) -> XStreamReadResult<Vec<u8>> {
        // Check stream state first
        self.check_readable()?;

        // Check for immediate error
        if let Some(error) = self.check_for_immediate_error().await {
            return Err(ErrorOnRead::xstream_error_only(error));
        }

        // For outbound streams, read with error awareness
        if self.direction == XStreamDirection::Outbound {
            self.read_to_end_with_error_awareness().await
        } else {
            // For inbound streams, simple read
            self.read_to_end_simple().await
        }
    }

    /// Simple read_to_end for inbound streams
    async fn read_to_end_simple(&self) -> XStreamReadResult<Vec<u8>> {
        let mut buf: Vec<u8> = Vec::new();

        match self.execute_main_read_op(|reader| {
            Box::pin(async move {
                let bytes_read = reader.read_to_end(&mut buf).await?;
                if bytes_read == 0 && buf.is_empty() {
                    debug!("Stream was already at EOF");
                }
                Ok(buf)
            })
        }).await {
            Ok(data) => Ok(data),
            Err(e) => Err(ErrorOnRead::io_error_only(e)),
        }
    }

    /// Read to end with error awareness for outbound streams
    async fn read_to_end_with_error_awareness(&self) -> XStreamReadResult<Vec<u8>> {
        let mut buf = Vec::new();
        let mut temp_buf = vec![0u8; 4096];

        loop {
            let stream_main_read = self.stream_main_read.clone();
            
            select! {
                // Try to read more data
                read_result = async {
                    let mut guard = stream_main_read.lock().await;
                    guard.read(&mut temp_buf).await
                } => {
                    match read_result {
                        Ok(0) => {
                            // EOF reached - normal completion
                            debug!("Read to end completed, total bytes: {}", buf.len());
                            return Ok(buf);
                        },
                        Ok(n) => {
                            buf.extend_from_slice(&temp_buf[0..n]);
                            debug!("Read {} bytes, total: {}", n, buf.len());
                        },
                        Err(e) => {
                            // IO error during read
                            self.state_manager.handle_connection_error(&e, "read_to_end error");
                            return Err(ErrorOnRead::from_io_error(buf, e));
                        }
                    }
                },
                // Wait for error from server
                error_result = self.error_data_store.wait_for_error() => {
                    match error_result {
                        Ok(error_data) => {
                            // Server sent an error
                            let xstream_error = XStreamError::new(error_data);
                            return Err(ErrorOnRead::from_xstream_error(buf, xstream_error));
                        },
                        Err(_) => {
                            // Error stream closed, continue reading until EOF
                            debug!("Error stream closed, continuing to read until EOF");
                        }
                    }
                }
            }
        }
    }

    /// Reads available data from the main stream with error awareness
    pub async fn read(&self) -> XStreamReadResult<Vec<u8>> {
        // Check stream state first
        self.check_readable()?;

        // Check for immediate error
        if let Some(error) = self.check_for_immediate_error().await {
            return Err(ErrorOnRead::xstream_error_only(error));
        }

        // For outbound streams, read with error awareness
        if self.direction == XStreamDirection::Outbound {
            self.read_with_error_awareness().await
        } else {
            // For inbound streams, simple read
            self.read_simple().await
        }
    }

    /// Simple read for inbound streams
    async fn read_simple(&self) -> XStreamReadResult<Vec<u8>> {
        let mut buf: Vec<u8> = vec![0; 4096];

        match self.execute_main_read_op(|reader| {
            Box::pin(async move {
                let bytes_read = reader.read(&mut buf).await?;
                if bytes_read == 0 {
                    debug!("Detected EOF while reading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "End of file",
                    ));
                }
                buf.truncate(bytes_read);
                Ok(buf)
            })
        }).await {
            Ok(data) => Ok(data),
            Err(e) => Err(ErrorOnRead::io_error_only(e)),
        }
    }

    /// Read with error awareness for outbound streams
    async fn read_with_error_awareness(&self) -> XStreamReadResult<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        let stream_main_read = self.stream_main_read.clone();

        select! {
            // Try to read data
            read_result = async {
                let mut guard = stream_main_read.lock().await;
                guard.read(&mut buf).await
            } => {
                match read_result {
                    Ok(0) => {
                        // EOF reached
                        let eof_error = std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "End of file"
                        );
                        Err(ErrorOnRead::io_error_only(eof_error))
                    },
                    Ok(n) => {
                        buf.truncate(n);
                        debug!("Read {} bytes", n);
                        Ok(buf)
                    },
                    Err(e) => {
                        self.state_manager.handle_connection_error(&e, "read error");
                        Err(ErrorOnRead::io_error_only(e))
                    }
                }
            },
            // Wait for error from server
            error_result = self.error_data_store.wait_for_error() => {
                match error_result {
                    Ok(error_data) => {
                        // Server sent an error
                        let xstream_error = XStreamError::new(error_data);
                        Err(ErrorOnRead::xstream_error_only(xstream_error))
                    },
                    Err(_) => {
                        // Error stream closed, perform normal read
                        debug!("Error stream closed, performing normal read");
                        self.read_simple().await
                    }
                }
            }
        }
    }

    // ===== CONVENIENCE METHODS FOR BACKWARD COMPATIBILITY =====

    /// Read ignoring XStream errors (backward compatibility)
    pub async fn read_ignore_errors(&self) -> Result<Vec<u8>, std::io::Error> {
        match self.read().await {
            Ok(data) => Ok(data),
            Err(error_on_read) => {
                if error_on_read.has_partial_data() {
                    // Return partial data if available
                    Ok(error_on_read.into_partial_data())
                } else {
                    // Convert to IO error
                    match error_on_read.into_error() {
                        ReadError::Io(io_wrapper) => Err(io_wrapper.to_io_error()),
                        ReadError::XStream(xs_error) => {
                            // Convert XStream error to IO error
                            Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("XStream error: {}", xs_error)
                            ))
                        }
                    }
                }
            }
        }
    }

    /// Read to end ignoring XStream errors (backward compatibility)
    pub async fn read_to_end_ignore_errors(&self) -> Result<Vec<u8>, std::io::Error> {
        match self.read_to_end().await {
            Ok(data) => Ok(data),
            Err(error_on_read) => {
                if error_on_read.has_partial_data() {
                    // Return partial data if available
                    Ok(error_on_read.into_partial_data())
                } else {
                    // Convert to IO error
                    match error_on_read.into_error() {
                        ReadError::Io(io_wrapper) => Err(io_wrapper.to_io_error()),
                        ReadError::XStream(xs_error) => {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("XStream error: {}", xs_error)
                            ))
                        }
                    }
                }
            }
        }
    }

    // ===== WRITE OPERATIONS (UNCHANGED) =====

    /// Writes all data to the main stream
    pub async fn write_all(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        self.execute_main_write_op(|writer| {
            let data = buf.clone();
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
    pub async fn write_eof(&self) -> Result<(), std::io::Error> {
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
                    writer.flush().await?;
                    writer.close().await?;
                    Ok(())
                })
            })
            .await;

        match result {
            Ok(_) => {
                debug!("Stream {:?} write half shutdown (EOF sent)", self.id);
                self.state_manager.mark_write_local_closed();
                Ok(())
            }
            Err(e) => {
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
    pub async fn error_read(&self) -> Result<Vec<u8>, std::io::Error> {
        if self.direction != XStreamDirection::Outbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only outbound streams can read from error stream",
            ));
        }

        debug!("Waiting for error data for stream {:?}", self.id);
        self.error_data_store.wait_for_error().await
    }

    /// Internal method to read from error stream (used by background task)
    pub async fn inner_error_read(&self) -> Result<Vec<u8>, std::io::Error> {
        if self.direction != XStreamDirection::Outbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only outbound streams can read from error stream",
            ));
        }

        // This method is for the background task, not for direct use
        debug!("Inner error read for stream {:?} - should not be called directly", self.id);
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "inner_error_read is for internal use only"
        ))
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
    /// This method also closes the main write stream and error write stream
    pub async fn error_write(&self, error_data: Vec<u8>) -> Result<(), std::io::Error> {
        if self.direction != XStreamDirection::Inbound {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Only inbound streams can write to error stream",
            ));
        }

        if self.state_manager.has_error_written() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Error already written to this stream",
            ));
        }

        // Mark that we're writing an error
        self.state_manager.mark_error_written();

        // Write the error data to the error stream
        let data_clone = error_data.clone();
        let error_write_result = self
            .execute_error_write_op(|writer| {
                let error_data = data_clone.clone();
                Box::pin(async move {
                    writer.write_all(&error_data).await?;
                    writer.flush().await?;
                    writer.close().await?; // Close error stream (EOF)
                    Ok(())
                })
            })
            .await;

        // Also close the main write stream (EOF)
        let main_write_result = self
            .execute_main_write_op(|writer| {
                Box::pin(async move {
                    writer.flush().await?;
                    writer.close().await?; // Close main write stream (EOF)
                    Ok(())
                })
            })
            .await;

        match (error_write_result, main_write_result) {
            (Ok(_), Ok(_)) => {
                debug!("Error successfully written to stream {:?} and streams closed", self.id);
                self.state_manager.mark_write_local_closed();
                self.state_manager.mark_error("Error written to error stream");
                Ok(())
            }
            (Err(e), _) | (_, Err(e)) => {
                error!("Failed to write error or close streams for stream {:?}: {:?}", self.id, e);
                self.state_manager
                    .handle_connection_error(&e, "error during error_write");
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

        // Если запись еще не закрыта, отправляем EOF для совместимости с QUIC
        if !self.state_manager.is_write_local_closed() {
            debug!("Stream {:?} write not closed, sending EOF before close", self.id);
            let _ = self.write_eof().await; // Игнорируем ошибки при закрытии
        }

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

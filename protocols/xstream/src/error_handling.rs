// error_handling.rs
// Module for handling XStream error reading with background tasks

use futures::AsyncReadExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot, watch};
use tracing::{debug, error, info, warn};

use super::types::{XStreamDirection, XStreamID};

/// Awaitable error data structure that can be shared between tasks
/// 
/// This structure allows multiple consumers to wait for error data
/// while a single background task reads from the error stream.
#[derive(Debug, Clone)]
pub struct ErrorDataStore {
    /// Receiver for error data - can be cloned and awaited from multiple places
    error_receiver: Arc<Mutex<Option<watch::Receiver<Option<Vec<u8>>>>>>,
    /// Sender for error data - used by background task
    error_sender: Arc<Mutex<Option<watch::Sender<Option<Vec<u8>>>>>>,
    /// Flag to indicate if error was already received
    error_received: Arc<tokio::sync::RwLock<bool>>,
    /// Cached error data for subsequent reads
    cached_error: Arc<Mutex<Option<Vec<u8>>>>,
}

impl ErrorDataStore {
    /// Create a new ErrorDataStore
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(None);
        
        Self {
            error_receiver: Arc::new(Mutex::new(Some(receiver))),
            error_sender: Arc::new(Mutex::new(Some(sender))),
            error_received: Arc::new(tokio::sync::RwLock::new(false)),
            cached_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Wait for error data to arrive or return cached data if already available
    /// 
    /// This method will:
    /// 1. Return cached data immediately if available
    /// 2. Wait for new error data if not cached
    /// 3. Cache the error data for future reads
    pub async fn wait_for_error(&self) -> Result<Vec<u8>, std::io::Error> {
        // First check if we have cached data
        {
            let cached = self.cached_error.lock().await;
            if let Some(ref data) = *cached {
                debug!("Returning cached error data ({} bytes)", data.len());
                return Ok(data.clone());
            }
        }

        // If no cached data, wait for new error
        let mut receiver_guard = self.error_receiver.lock().await;
        if let Some(receiver) = receiver_guard.as_mut() {
            debug!("Waiting for error data to arrive...");
            
            // Wait for error data to arrive
            loop {
                match receiver.changed().await {
                    Ok(_) => {
                        let current_value = receiver.borrow().clone();
                        if let Some(error_data) = current_value {
                            debug!("Error data received ({} bytes)", error_data.len());
                            
                            // Cache the error for future reads
                            {
                                let mut cached = self.cached_error.lock().await;
                                *cached = Some(error_data.clone());
                            }
                            
                            // Mark as received
                            {
                                let mut received = self.error_received.write().await;
                                *received = true;
                            }
                            
                            debug!("Error data cached and marked as received");
                            return Ok(error_data);
                        }
                        // Continue waiting if we got None
                    }
                    Err(_) => {
                        // Sender was dropped, no error will come
                        debug!("Error sender was dropped - no error data will arrive");
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "Error stream closed without receiving error data",
                        ));
                    }
                }
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Error receiver not available",
            ));
        }
    }

    /// Store error data (used by background task)
    /// 
    /// This method sends error data to all waiting consumers
    pub async fn store_error(&self, data: Vec<u8>) -> Result<(), std::io::Error> {
        // Check if error was already received
        {
            let received = self.error_received.read().await;
            if *received {
                debug!("Error already received, ignoring new error data");
                return Ok(());
            }
        }

        debug!("Storing error data ({} bytes)", data.len());

        // Send error data through watch channel
        let sender_guard = self.error_sender.lock().await;
        if let Some(ref sender) = *sender_guard {
            match sender.send(Some(data.clone())) {
                Ok(_) => {
                    debug!("Error data sent to watchers");
                    
                    // Also cache it immediately
                    {
                        let mut cached = self.cached_error.lock().await;
                        *cached = Some(data);
                    }
                    
                    // Mark as received
                    {
                        let mut received = self.error_received.write().await;
                        *received = true;
                    }
                    
                    debug!("Error data stored successfully");
                    Ok(())
                }
                Err(_) => {
                    error!("Failed to send error data - no receivers");
                    Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Failed to send error data - no receivers",
                    ))
                }
            }
        } else {
            error!("Error sender not available");
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Error sender not available",
            ))
        }
    }

    /// Check if error data is available without waiting
    pub async fn has_error(&self) -> bool {
        let received = self.error_received.read().await;
        *received
    }

    /// Get cached error data if available (non-blocking)
    pub async fn get_cached_error(&self) -> Option<Vec<u8>> {
        let cached = self.cached_error.lock().await;
        cached.clone()
    }

    /// Close the error data store (used when stream is closing)
    /// 
    /// This will signal to all waiters that no more error data will arrive
    pub async fn close(&self) {
        debug!("Closing ErrorDataStore");
        
        // Drop the sender to signal no more data will come
        let mut sender_guard = self.error_sender.lock().await;
        *sender_guard = None;
        
        debug!("ErrorDataStore closed");
    }

    /// Check if the store is closed
    pub async fn is_closed(&self) -> bool {
        let sender_guard = self.error_sender.lock().await;
        sender_guard.is_none()
    }

    /// Clear cached error data (useful for testing)
    pub async fn clear_cache(&self) {
        let mut cached = self.cached_error.lock().await;
        *cached = None;
        
        let mut received = self.error_received.write().await;
        *received = false;
        
        debug!("Error cache cleared");
    }
}

impl Default for ErrorDataStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Background error reading task manager
/// 
/// This manages a background tokio task that reads from the error stream
/// and stores the data in an ErrorDataStore for consumers to await.
pub struct ErrorReaderTask {
    /// Task handle for the background reader
    task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender
    shutdown_sender: Option<oneshot::Sender<()>>,
    /// Stream information for logging
    stream_id: XStreamID,
    /// Whether the task is running
    is_running: bool,
}

impl ErrorReaderTask {
    /// Start background error reading task
    /// 
    /// This creates and starts a background task that will:
    /// 1. Read all data from the error stream
    /// 2. Store it in the ErrorDataStore
    /// 3. Handle graceful shutdown when signaled
    /// 
    /// # Arguments
    /// * `stream_id` - ID of the XStream for logging
    /// * `peer_id` - Peer ID for notifications
    /// * `direction` - Stream direction (only outbound streams read errors)
    /// * `error_stream` - The error stream to read from
    /// * `error_data_store` - Store to save error data
    /// * `closure_notifier` - Channel to notify about stream closure
    pub fn start(
        stream_id: XStreamID,
        peer_id: PeerId,
        direction: XStreamDirection,
        error_stream: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
        error_data_store: ErrorDataStore,
        closure_notifier: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    ) -> Self {
        let (shutdown_sender, mut shutdown_receiver) = oneshot::channel::<()>();

        info!("Starting error reader task for stream {:?}", stream_id);

        let task_handle = tokio::spawn(async move {
            debug!("Error reader task started for stream {:?}", stream_id);

            // Only outbound streams should read from error stream
            if direction != XStreamDirection::Outbound {
                debug!("Inbound stream - not reading from error stream");
                return;
            }

            let mut error_data_store = error_data_store;

            tokio::select! {
                // Handle shutdown signal
                _ = shutdown_receiver => {
                    debug!("Error reader task for stream {:?} received shutdown signal", stream_id);
                    error_data_store.close().await;
                    return;
                }
                
                // Read error from stream
                result = Self::read_error_from_stream(stream_id, error_stream, &error_data_store) => {
                    match result {
                        Ok(bytes_read) => {
                            if bytes_read > 0 {
                                info!("Error reader task for stream {:?} completed - read {} bytes", stream_id, bytes_read);
                            } else {
                                debug!("Error reader task for stream {:?} completed - no error data", stream_id);
                            }
                        }
                        Err(e) => {
                            error!("Error reader task for stream {:?} failed: {:?}", stream_id, e);
                        }
                    }
                }
            }

            // Close the error data store when task ends
            error_data_store.close().await;
            
            // Notify about stream closure if connection was lost
            if let Err(e) = closure_notifier.send((peer_id, stream_id)) {
                debug!("Failed to send closure notification for stream {:?}: {:?}", stream_id, e);
            }
            
            debug!("Error reader task for stream {:?} exiting", stream_id);
        });

        Self {
            task_handle: Some(task_handle),
            shutdown_sender: Some(shutdown_sender),
            stream_id,
            is_running: true,
        }
    }

    /// Internal method to read error from stream
    /// 
    /// This method reads all available data from the error stream
    /// and stores it in the ErrorDataStore.
    async fn read_error_from_stream(
        stream_id: XStreamID,
        error_stream: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
        error_data_store: &ErrorDataStore,
    ) -> Result<usize, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        
        debug!("Starting to read from error stream for stream {:?}", stream_id);
        
        // Lock the error stream and read all data
        let mut stream_guard = error_stream.lock().await;
        
        match stream_guard.read_to_end(&mut buf).await {
            Ok(bytes_read) => {
                debug!(
                    "Read {} bytes from error stream for stream {:?}",
                    bytes_read, stream_id
                );
                
                if !buf.is_empty() {
                    // Store the error data
                    error_data_store.store_error(buf).await?;
                    debug!("Error data stored successfully for stream {:?}", stream_id);
                } else {
                    debug!("Error stream closed without data for stream {:?}", stream_id);
                }
                
                Ok(bytes_read)
            }
            Err(e) => {
                error!(
                    "Failed to read from error stream for stream {:?}: {:?}",
                    stream_id, e
                );
                Err(e)
            }
        }
    }

    /// Get the stream ID this task is managing
    pub fn stream_id(&self) -> XStreamID {
        self.stream_id
    }

    /// Check if the task is still running
    pub fn is_running(&self) -> bool {
        self.is_running && 
        self.task_handle.as_ref().map_or(false, |h| !h.is_finished())
    }

    /// Check if the task has finished
    pub fn is_finished(&self) -> bool {
        self.task_handle.as_ref().map_or(true, |h| h.is_finished())
    }

    /// Shutdown the background task gracefully
    /// 
    /// This method will:
    /// 1. Send a shutdown signal to the task
    /// 2. Wait for the task to complete with a timeout
    /// 3. Handle any errors during shutdown
    pub async fn shutdown(mut self) {
        if !self.is_running {
            debug!("Error reader task for stream {:?} already shut down", self.stream_id);
            return;
        }

        debug!("Shutting down error reader task for stream {:?}", self.stream_id);
        
        // Send shutdown signal
        if let Some(sender) = self.shutdown_sender.take() {
            if let Err(_) = sender.send(()) {
                debug!("Failed to send shutdown signal - task may have already finished");
            }
        }

        // Wait for task to complete with timeout
        if let Some(handle) = self.task_handle.take() {
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(result) => {
                    match result {
                        Ok(_) => {
                            debug!("Error reader task for stream {:?} shut down successfully", self.stream_id);
                        }
                        Err(e) => {
                            error!("Error reader task for stream {:?} failed during shutdown: {:?}", self.stream_id, e);
                        }
                    }
                }
                Err(_) => {
                    error!("Error reader task for stream {:?} shutdown timed out", self.stream_id);
                }
            }
        }
        
        self.is_running = false;
        debug!("Error reader task shutdown complete for stream {:?}", self.stream_id);
    }

    /// Force abort the background task without waiting
    /// 
    /// This should only be used when graceful shutdown is not possible
    pub fn abort(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            warn!("Force aborting error reader task for stream {:?}", self.stream_id);
            handle.abort();
        }
        
        self.is_running = false;
        
        // Drop shutdown sender to signal task should stop
        self.shutdown_sender = None;
    }
}

impl Drop for ErrorReaderTask {
    fn drop(&mut self) {
        if self.is_running && (self.task_handle.is_some() || self.shutdown_sender.is_some()) {
            warn!("ErrorReaderTask for stream {:?} dropped without calling shutdown()", self.stream_id);
            
            // Send shutdown signal if still available
            if let Some(sender) = self.shutdown_sender.take() {
                let _ = sender.send(());
            }
            
            // Abort the task if still running
            if let Some(handle) = self.task_handle.take() {
                handle.abort();
            }
            
            self.is_running = false;
        }
    }
}

impl std::fmt::Debug for ErrorReaderTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorReaderTask")
            .field("stream_id", &self.stream_id)
            .field("is_running", &self.is_running)
            .field("has_handle", &self.task_handle.is_some())
            .field("has_shutdown_sender", &self.shutdown_sender.is_some())
            .finish()
    }
}
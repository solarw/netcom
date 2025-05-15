use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use tracing::{debug, info, warn, error};

use super::types::{XStreamID, XStreamState, XStreamDirection};

/// XStream struct - represents a pair of streams for data transfer
#[derive(Debug)]
pub struct XStream {
    pub stream_main_read: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
    pub stream_main_write: Arc<tokio::sync::Mutex<futures::io::WriteHalf<Stream>>>,
    pub id: XStreamID,
    pub peer_id: PeerId,
    // Направление потока (входящий или исходящий)
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
        direction: XStreamDirection,
        closure_notifier: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    ) -> Self {
        info!("Creating new XStream with id: {:?} for peer: {}, direction: {:?}", id, peer_id, direction);
        Self {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
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
            
            // If local closed and remote closes, become fully closed
            (XStreamState::LocalClosed, XStreamState::RemoteClosed) => XStreamState::FullyClosed,
            
            // If remote closed and local closes, become fully closed
            (XStreamState::RemoteClosed, XStreamState::LocalClosed) => XStreamState::FullyClosed,
            
            // Otherwise, use the new state
            (_, new_state) => new_state,
        };
        
        // Update the state
        self.state.store(final_state as u8, Ordering::Release);
        info!("Stream {:?} state changed: {:?} -> {:?}", self.id, current_state, final_state);
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
            XStreamState::Open => false,
            _ => true,
        }
    }

    /// Check if the stream is closed locally
    pub fn is_local_closed(&self) -> bool {
        matches!(self.state(), XStreamState::LocalClosed | XStreamState::FullyClosed)
    }

    /// Check if the stream is closed remotely 
    pub fn is_remote_closed(&self) -> bool {
        matches!(self.state(), XStreamState::RemoteClosed | XStreamState::FullyClosed)
    }

    /// Helper method to send closure notification when EOF or connection error is detected
    fn notify_eof(&self, reason: &str) {
        // Mark as remotely closed first
        self.mark_remote_closed();
        
        info!("Sending closure notification for stream {:?} due to: {}", self.id, reason);
        
        match self.closure_notifier.send((self.peer_id, self.id)) {
            Ok(_) => info!("Closure notification sent successfully for stream {:?}", self.id),
            Err(e) => warn!("Failed to send closure notification for stream {:?}: {}", self.id, e),
        }
    }

    /// Validate that the stream is in a valid state for reading
    fn check_readable(&self) -> Result<(), std::io::Error> {
        if self.is_remote_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Cannot read from stream {:?}: remotely closed", self.id)
            ));
        }
        Ok(())
    }

    /// Validate that the stream is in a valid state for writing
    fn check_writable(&self) -> Result<(), std::io::Error> {
        if self.is_local_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Cannot write to stream {:?}: locally closed", self.id)
            ));
        }
        if self.is_remote_closed() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Cannot write to stream {:?}: remotely closed", self.id)
            ));
        }
        Ok(())
    }

    /// Checks if an error indicates that the connection was closed by remote
    fn is_connection_closed_error(&self, e: &std::io::Error) -> bool {
        matches!(
            e.kind(),
            std::io::ErrorKind::BrokenPipe |
            std::io::ErrorKind::ConnectionReset |
            std::io::ErrorKind::ConnectionAborted
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
                if e.kind() == std::io::ErrorKind::UnexpectedEof || self.is_connection_closed_error(&e) {
                    info!("Detected EOF/connection error while reading exact bytes from stream {:?}: {:?}", self.id, e.kind());
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
                    self.notify_eof("read_to_end returned 0 bytes");
                }
                self.notify_eof("read_to_end all");
                Ok(buf)
                
            },
            Err(e) => {
                if self.is_connection_closed_error(&e) {
                    info!("Detected connection error during read_to_end for stream {:?}: {:?}", self.id, e.kind());
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
        
        let mut buf: Vec<u8> = Vec::new();
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
                    self.notify_eof("read returned 0 bytes");
                    
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Remote closed connection"
                    ));
                }
                
                Ok(buf)
            },
            Err(e) => {
                if self.is_connection_closed_error(&e) {
                    info!("Detected connection error during read for stream {:?}: {:?}", self.id, e.kind());
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
                            info!("Detected remote closure during flush for stream {:?}", self.id);
                            self.notify_eof(&format!("flush error: {:?}", e.kind()));
                        }
                        Err(e)
                    }
                }
            },
            Err(e) => {
                // Check if the error indicates a broken pipe or connection reset
                if self.is_connection_closed_error(&e) {
                    info!("Detected remote closure during write for stream {:?}", self.id);
                    self.notify_eof(&format!("write error: {:?}", e.kind()));
                }
                Err(e)
            }
        }
    }

    /// Closes the streams
    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        info!("Closing XStream with id: {:?} for peer: {}", self.id, self.peer_id);
        
        // If already closed locally, return early
        if self.is_local_closed() {
            debug!("Stream {:?} already locally closed", self.id);
            return Ok(());
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
                                self.mark_remote_closed();
                                // Return success if remote already closed it
                                Ok(())
                            } else {
                                Err(e)
                            }
                        }
                    }
                },
                Err(e) => {
                    // Check if error indicates the stream was already closed by remote
                    if self.is_connection_closed_error(&e) {
                        info!("Remote already closed connection during flush for stream {:?}", self.id);
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
        
        // Send closure notification
        debug!("Sending closure notification for stream {:?} of peer {}", self.id, self.peer_id);
        
        // This is non-blocking and returns immediately
        match self.closure_notifier.send((self.peer_id, self.id)) {
            Ok(_) => debug!("Close notification sent successfully for stream {:?}", self.id),
            Err(e) => warn!("Failed to send close notification for stream {:?}: {}", self.id, e),
        }
        
        // Add a debug print just before returning
        debug!("Stream close complete for {:?} - state: {:?}, result: {:?}", 
               self.id, self.state(), result);
        result
    }
}

impl Clone for XStream {
    fn clone(&self) -> Self {
        debug!("Cloning XStream with id: {:?} for peer: {}", self.id, self.peer_id);
        
        // Clone the stream with all its components
        Self {
            stream_main_read: self.stream_main_read.clone(),
            stream_main_write: self.stream_main_write.clone(),
            id: self.id,
            peer_id: self.peer_id,
            direction: self.direction,
            closure_notifier: self.closure_notifier.clone(),
            state: self.state.clone(),
        }
    }
}
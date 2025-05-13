use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

/// XStream struct - represents a pair of streams for data transfer
#[derive(Debug)]
pub struct XStream {
    pub stream_main_read: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
    pub stream_main_write: Arc<tokio::sync::Mutex<futures::io::WriteHalf<Stream>>>,
    pub id: u128,
    pub peer_id: PeerId,
    // Simple Option for closure notifier
    closure_notifier: Option<mpsc::UnboundedSender<(PeerId, u128)>>,
}

impl XStream {
    /// Creates a new XStream from components
    pub fn new(
        id: u128,
        peer_id: PeerId,
        stream_main_read: futures::io::ReadHalf<Stream>,
        stream_main_write: futures::io::WriteHalf<Stream>,
    ) -> Self {
        debug!("Creating new XStream with id: {} for peer: {}", id, peer_id);
        Self {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
            id,
            peer_id,
            closure_notifier: None,
        }
    }

    /// Check if a closure notifier is set
    pub fn has_closure_notifier(&self) -> bool {
        self.closure_notifier.is_some()
    }

    /// Set a closure notifier
    pub fn set_closure_notifier(&mut self, notifier: mpsc::UnboundedSender<(PeerId, u128)>) {
        debug!("Setting closure notifier for stream {}", self.id);
        self.closure_notifier = Some(notifier);
    }

    /// Reads exact number of bytes from the main stream
    pub async fn read_exact(&self, size: usize) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = vec![0u8; size];
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read_exact(&mut buf).await?;
        Ok(buf)
    }

    /// Reads all data from the main stream to the end
    pub async fn read_to_end(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    /// Reads available data from the main stream
    pub async fn read(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read(&mut buf).await?;
        Ok(buf)
    }

    /// Writes all data to the main stream
    pub async fn write_all(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        let stream_main_write = self.stream_main_write.clone();
        let mut unlocked = stream_main_write.lock().await;
        unlocked.write_all(&buf).await?;
        unlocked.flush().await
    }

    /// Closes the streams
    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        info!("Closing XStream with id: {} for peer: {}", self.id, self.peer_id);
        
        if self.has_closure_notifier() {
            debug!("Stream {} has closure notifier before closing", self.id);
        } else {
            warn!("No closure notifier set for stream {} of peer {} before closing", self.id, self.peer_id);
        }
        
        // Get a lock on the write stream
        let stream_main_write = self.stream_main_write.clone();
        let mut unlocked = stream_main_write.lock().await;
        
        // Flush first
        unlocked.flush().await?;
        
        // Then close
        let result = unlocked.close().await;
        debug!("Network stream close result: {:?}", result);
        
        debug!("close done!!!!!");
        debug!("quic close done1111");
        
        // Send notification if notifier is set
        if let Some(notifier) = &self.closure_notifier {
            debug!("Sending closure notification for stream {} of peer {}", self.id, self.peer_id);
            
            // This is non-blocking and returns immediately
            match notifier.send((self.peer_id, self.id)) {
                Ok(_) => debug!("Close notification sent successfully for stream {}", self.id),
                Err(e) => warn!("Failed to send close notification for stream {}: {}", self.id, e),
            }
        } else {
            warn!("No closure notifier set for stream {} of peer {}", self.id, self.peer_id);
        }
        
        result
    }
}

impl Clone for XStream {
    fn clone(&self) -> Self {
        debug!("Cloning XStream with id: {} for peer: {}", self.id, self.peer_id);
        
        // IMPORTANT: Preserve the closure notifier when cloning
        let clone = Self {
            stream_main_read: self.stream_main_read.clone(),
            stream_main_write: self.stream_main_write.clone(),
            id: self.id,
            peer_id: self.peer_id,
            closure_notifier: self.closure_notifier.clone(),
        };
        
        if clone.has_closure_notifier() {
            debug!("Closure notifier was preserved in clone for stream {}", self.id);
        } else {
            warn!("Closure notifier NOT preserved in clone for stream {}", self.id);
        }
        
        clone
    }
}
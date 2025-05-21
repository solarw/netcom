// xstream_state.rs
// Module for managing XStream states, transitions, and notifications

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

use super::types::{XStreamDirection, XStreamID, XStreamState};
use libp2p::PeerId;

/// Manages the state of an XStream with thread-safe transitions and notifications
#[derive(Debug)]
pub struct XStreamStateManager {
    /// Current state stored as atomic for thread safety
    state: Arc<AtomicU8>,
    /// Stream ID for identification and logging
    stream_id: XStreamID,
    /// Peer ID for identification and notifications
    peer_id: PeerId,
    /// Direction of the stream (inbound or outbound)
    direction: XStreamDirection,
    /// Closure notifier for sending state change events
    closure_notifier: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    /// Saved error data, if any was received
    error_data: Arc<Mutex<Option<Vec<u8>>>>,
    /// Flag indicating that an error was written
    error_written: Arc<AtomicU8>,
}

impl XStreamStateManager {
    /// Creates a new state manager starting in Open state
    pub fn new(
        stream_id: XStreamID,
        peer_id: PeerId,
        direction: XStreamDirection,
        closure_notifier: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    ) -> Self {
        Self {
            state: Arc::new(AtomicU8::new(XStreamState::Open as u8)),
            stream_id,
            peer_id,
            direction,
            closure_notifier,
            error_data: Arc::new(Mutex::new(None)),
            error_written: Arc::new(AtomicU8::new(0)),
        }
    }

    /// Get the current state
    pub fn state(&self) -> XStreamState {
        let value = self.state.load(Ordering::Acquire);
        XStreamState::from(value)
    }

    /// Set the stream state with appropriate transition rules
    pub fn set_state(&self, new_state: XStreamState) {
        let current_state = self.state();

        // Apply state transition rules
        let final_state = match (current_state, new_state) {
            // If already fully closed, stay closed
            (XStreamState::FullyClosed, _) => XStreamState::FullyClosed,

            // If write locally closed and read remotely closed, become fully closed
            (XStreamState::WriteLocalClosed, XStreamState::ReadRemoteClosed) => {
                XStreamState::FullyClosed
            }
            (XStreamState::ReadRemoteClosed, XStreamState::WriteLocalClosed) => {
                XStreamState::FullyClosed
            }

            // If local closed and remote closes, become fully closed
            (XStreamState::LocalClosed, XStreamState::RemoteClosed) => XStreamState::FullyClosed,

            // If remote closed and local closes, become fully closed
            (XStreamState::RemoteClosed, XStreamState::LocalClosed) => XStreamState::FullyClosed,

            // Otherwise, use the new state
            (_, new_state) => new_state,
        };

        // Update the state
        if current_state != final_state {
            self.state.store(final_state as u8, Ordering::Release);
            debug!(
                "Stream {:?} state changed: {:?} -> {:?}",
                self.stream_id, current_state, final_state
            );

            // Send notifications for certain transitions
            if final_state == XStreamState::FullyClosed
                || final_state == XStreamState::Error
                || new_state == XStreamState::ReadRemoteClosed
                || new_state == XStreamState::RemoteClosed
            {
                // These state transitions should trigger a notification
                self.notify_state_change(&format!("State transition to {:?}", final_state));
            }
        }
    }

    /// Send notification about state change
    pub fn notify_state_change(&self, reason: &str) {
        debug!(
            "Sending state change notification for stream {:?} due to: {}",
            self.stream_id, reason
        );
        match self.closure_notifier.send((self.peer_id, self.stream_id)) {
            Ok(_) => debug!(
                "State change notification sent successfully for stream {:?}",
                self.stream_id
            ),
            Err(e) => warn!(
                "Failed to send state change notification for stream {:?}: {}",
                self.stream_id, e
            ),
        }
    }

    /// Mark the stream as write locally closed (EOF sent)
    pub fn mark_write_local_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::WriteLocalClosed),
            XStreamState::ReadRemoteClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already in a more restrictive closed state
        }
    }

    /// Mark the stream as read remotely closed (EOF received)
    pub fn mark_read_remote_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::ReadRemoteClosed),
            XStreamState::WriteLocalClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already in a more restrictive closed state
        }
    }

    /// Mark the stream as locally closed
    pub fn mark_local_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open | XStreamState::WriteLocalClosed => {
                // Explicitly handle the WriteLocalClosed -> LocalClosed transition
                self.set_state(XStreamState::LocalClosed);

                // Ensure notification is sent for this important transition
                self.notify_state_change("Stream marked as locally closed");
            }
            XStreamState::RemoteClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already locally closed or fully closed
        }
    }

    /// Mark the stream as remotely closed
    pub fn mark_remote_closed(&self) {
        let current = self.state();
        match current {
            XStreamState::Open => self.set_state(XStreamState::RemoteClosed),
            XStreamState::LocalClosed => self.set_state(XStreamState::FullyClosed),
            _ => {} // Already remotely closed or fully closed
        }
    }

    /// Mark the stream as errored
    pub fn mark_error(&self, reason: &str) {
        self.set_state(XStreamState::Error);
        self.notify_state_change(&format!("Error: {}", reason));
    }

    /// Check if the stream is closed (either locally, remotely, or both)
    pub fn is_closed(&self) -> bool {
        matches!(
            self.state(),
            XStreamState::LocalClosed
                | XStreamState::RemoteClosed
                | XStreamState::FullyClosed
                | XStreamState::Error
        )
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

    /// Checks if an error was written
    pub fn has_error_written(&self) -> bool {
        self.error_written.load(Ordering::Acquire) == 1
    }

    /// Marks that an error was written
    pub fn mark_error_written(&self) {
        self.error_written.store(1, Ordering::Release);
    }

    /// Store error data received from the error stream
    pub async fn store_error_data(&self, data: Vec<u8>) {
        let mut error_guard = self.error_data.lock().await;
        if error_guard.is_none() {
            *error_guard = Some(data);
        }
    }

    /// Get stored error data, if any
    pub async fn get_error_data(&self) -> Option<Vec<u8>> {
        let error_guard = self.error_data.lock().await;
        error_guard.clone()
    }

    /// Check if error data is available
    pub async fn has_error_data(&self) -> bool {
        let error_guard = self.error_data.lock().await;
        error_guard.is_some()
    }

    /// Handle connection error with consistent notification
    pub fn handle_connection_error(&self, error: &std::io::Error, context: &str) -> bool {
        // Check if this is a connection error
        if self.is_connection_closed_error(error) {
            self.mark_remote_closed();
            self.notify_state_change(&format!("{}: {:?}", context, error.kind()));
            return true;
        }
        false
    }

    /// Checks if an error indicates that the connection was closed by remote
    pub fn is_connection_closed_error(&self, e: &std::io::Error) -> bool {
        matches!(
            e.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
        )
    }

    /// Get the direction of the stream
    pub fn direction(&self) -> XStreamDirection {
        self.direction
    }

    /// Get the stream ID
    pub fn stream_id(&self) -> XStreamID {
        self.stream_id
    }

    /// Get the peer ID
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }
}

impl Clone for XStreamStateManager {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            stream_id: self.stream_id,
            peer_id: self.peer_id,
            direction: self.direction,
            closure_notifier: self.closure_notifier.clone(),
            error_data: self.error_data.clone(),
            error_written: self.error_written.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;
    use std::time::Duration;
    use tokio::runtime::Runtime;

    #[test]
    fn test_state_transitions() {
        // Create a test runtime
        let rt = Runtime::new().unwrap();

        // Run the async test
        rt.block_on(async {
            // Create a test channel
            let (tx, mut rx) = mpsc::unbounded_channel();

            // Create a random peer ID
            let keypair = identity::Keypair::generate_ed25519();
            let peer_id = keypair.public().to_peer_id();

            // Create a stream ID
            let stream_id = XStreamID::from(1u128);

            // Create a state manager
            let manager = XStreamStateManager::new(
                stream_id,
                peer_id.clone(),
                XStreamDirection::Outbound,
                tx,
            );

            // Initial state should be Open
            assert_eq!(manager.state(), XStreamState::Open);

            // Mark write locally closed
            manager.mark_write_local_closed();
            assert_eq!(manager.state(), XStreamState::WriteLocalClosed);

            // Mark read remotely closed, should become fully closed
            manager.mark_read_remote_closed();
            assert_eq!(manager.state(), XStreamState::FullyClosed);

            // We should have received a notification for this transition
            let timeout = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
            assert!(timeout.is_ok(), "Should have received a notification");

            if let Ok(Some((notif_peer_id, notif_stream_id))) = timeout {
                assert_eq!(notif_peer_id, peer_id);
                assert_eq!(notif_stream_id, stream_id);
            }

            // Try to change state after fully closed, should remain fully closed
            manager.mark_local_closed();
            assert_eq!(manager.state(), XStreamState::FullyClosed);
        });
    }

    #[test]
    fn test_error_handling() {
        // Create a test runtime
        let rt = Runtime::new().unwrap();

        // Run the async test
        rt.block_on(async {
            // Create a test channel
            let (tx, mut rx) = mpsc::unbounded_channel();

            // Create a random peer ID
            let keypair = identity::Keypair::generate_ed25519();
            let peer_id = keypair.public().to_peer_id();

            // Create a stream ID
            let stream_id = XStreamID::from(2u128);

            // Create a state manager
            let manager =
                XStreamStateManager::new(stream_id, peer_id.clone(), XStreamDirection::Inbound, tx);

            // Test handling connection errors
            let broken_pipe = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Broken pipe");

            assert!(manager.is_connection_closed_error(&broken_pipe));

            // Handle a connection error
            let handled = manager.handle_connection_error(&broken_pipe, "test error");
            assert!(handled);

            // State should be RemoteClosed
            assert_eq!(manager.state(), XStreamState::RemoteClosed);

            // We should have received a notification
            let timeout = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
            assert!(timeout.is_ok(), "Should have received a notification");

            // Test non-connection errors
            let other_error = std::io::Error::new(std::io::ErrorKind::Other, "Some other error");

            assert!(!manager.is_connection_closed_error(&other_error));

            let handled = manager.handle_connection_error(&other_error, "test error");
            assert!(!handled);
        });
    }
}

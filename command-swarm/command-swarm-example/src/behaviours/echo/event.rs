//! Events for Echo behaviour

use libp2p::PeerId;

/// Events for echo behaviour
#[derive(Debug, Clone)]
pub enum EchoEvent {
    /// Message received from peer
    MessageReceived { peer_id: PeerId, text: String },
}

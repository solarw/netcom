//! Commands for Echo behaviour

use libp2p::PeerId;

/// Commands for echo behaviour
#[derive(Debug, Clone)]
pub enum EchoCommand {
    /// Send message to peer
    SendMessage { peer_id: PeerId, text: String },
}

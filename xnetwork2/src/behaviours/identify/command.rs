//! Identify commands for XNetwork2

use libp2p::PeerId;

/// Commands for Identify behaviour
#[derive(Debug, Clone)]
pub enum IdentifyCommand {
    /// Request identify information from a peer
    RequestIdentify {
        peer_id: PeerId,
    },
    /// Send identify information to a peer
    SendIdentify {
        peer_id: PeerId,
    },
}

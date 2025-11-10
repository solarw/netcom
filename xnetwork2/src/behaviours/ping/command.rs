//! Ping commands for XNetwork2

use libp2p::PeerId;

/// Commands for Ping behaviour
#[derive(Debug, Clone)]
pub enum PingCommand {
    /// Send ping to a peer
    SendPing { peer_id: PeerId },
}

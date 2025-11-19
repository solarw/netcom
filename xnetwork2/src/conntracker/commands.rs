//! Commands for Conntracker service

use libp2p::{PeerId, swarm::ConnectionId};
use tokio::sync::oneshot;

use super::{ConnectionInfo, PeerConnections, ConnectionStats};

/// Commands for Conntracker service
#[derive(Debug)]
pub enum ConntrackerCommand {
    /// Get all connections
    GetConnections {
        response: oneshot::Sender<Result<Vec<ConnectionInfo>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get connections for a specific peer
    GetPeerConnections {
        peer_id: PeerId,
        response: oneshot::Sender<Result<PeerConnections, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get information about a specific connection
    GetConnection {
        connection_id: ConnectionId,
        response: oneshot::Sender<Result<ConnectionInfo, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get all connected peers
    GetConnectedPeers {
        response: oneshot::Sender<Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get connection statistics
    GetConnectionStats {
        response: oneshot::Sender<Result<ConnectionStats, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get listen addresses
    GetListenAddresses {
        response: oneshot::Sender<Result<Vec<libp2p::Multiaddr>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get external addresses
    GetExternalAddresses {
        response: oneshot::Sender<Result<Vec<libp2p::Multiaddr>, Box<dyn std::error::Error + Send + Sync>>>,
    },
}

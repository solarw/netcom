//! Commands for ConnectionTracker behaviour

use libp2p::PeerId;
use tokio::sync::oneshot;

use crate::connection_tracker::{ConnectionInfo, PeerConnections, ConnectionStats};

/// Commands for ConnectionTracker behaviour
#[derive(Debug)]
pub enum ConnectionTrackerCommand {
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
        connection_id: command_swarm::ConnectionId,
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

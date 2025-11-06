//! Swarm-level commands for XNetwork2

use libp2p::{Multiaddr, PeerId};
use tokio::sync::oneshot;

use std::fmt;

/// Swarm-level commands for XNetwork2 with response channels
pub enum SwarmLevelCommand {
    /// Dial a peer
    Dial {
        peer_id: PeerId,
        addr: Multiaddr,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Listen on an address
    ListenOn {
        addr: Multiaddr,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Disconnect from a peer
    Disconnect {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get network state
    GetNetworkState {
        response: oneshot::Sender<Result<NetworkState, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Shutdown the node
    Shutdown {
        stopper: command_swarm::SwarmLoopStopper,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Echo command for testing - returns the same message back
    Echo {
        message: String,
        response: oneshot::Sender<Result<String, Box<dyn std::error::Error + Send + Sync>>>,
    },
}

/// Network state information
#[derive(Debug, Clone)]
pub struct NetworkState {
    pub peer_id: PeerId,
    pub listening_addresses: Vec<Multiaddr>,
    pub connected_peers: Vec<PeerId>,
    pub authenticated_peers: Vec<PeerId>,
}

impl fmt::Debug for SwarmLevelCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SwarmLevelCommand::Dial { peer_id, addr, .. } => {
                write!(f, "Dial(peer_id: {}, addr: {})", peer_id, addr)
            }
            SwarmLevelCommand::ListenOn { addr, .. } => {
                write!(f, "ListenOn(addr: {})", addr)
            }
            SwarmLevelCommand::Disconnect { peer_id, .. } => {
                write!(f, "Disconnect(peer_id: {})", peer_id)
            }
            SwarmLevelCommand::GetNetworkState { .. } => {
                write!(f, "GetNetworkState")
            }
            SwarmLevelCommand::Shutdown { .. } => {
                write!(f, "Shutdown")
            }
            SwarmLevelCommand::Echo { message, .. } => {
                write!(f, "Echo(message: '{}')", message)
            }
        }
    }
}

// src/commands.rs

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};
use std::error::Error;
use tokio::sync::oneshot;
use xauth::definitions::AuthResult;
use xstream::xstream::XStream;

// Import XRoutes commands
use crate::xroutes::XRoutesCommand;

// Import connection management types
use crate::connection_management::{ConnectionInfo, PeerInfo, NetworkState};

#[derive(Debug)]
pub enum NetworkCommand {
    // Core connection commands
    OpenListenPort {
        host: String,
        port: u16,
        response: oneshot::Sender<Result<Multiaddr, Box<dyn Error + Send + Sync>>>,
    },
    ListenOn {
        addr: Multiaddr,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    Connect {
        addr: Multiaddr,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    Disconnect {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },

    // Core stream commands
    OpenStream {
        peer_id: PeerId,
        connection_id: Option<ConnectionId>,
        response: oneshot::Sender<Result<XStream, String>>,
    },

    // Core status commands (legacy)
    GetConnectionsForPeer {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<Multiaddr>>,
    },
    GetPeersConnected {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<(PeerId, Vec<Multiaddr>)>>,
    },
    GetListenAddresses {
        response: oneshot::Sender<Vec<Multiaddr>>,
    },

    // Core authentication commands
    IsPeerAuthenticated {
        peer_id: PeerId,
        response: oneshot::Sender<bool>,
    },
    SubmitPorVerification {
        connection_id: ConnectionId,
        result: AuthResult,
    },
    
    // NEW: Enhanced authentication commands
    GetAuthenticatedPeers {
        response: oneshot::Sender<Vec<PeerId>>,
    },
    GetAuthMetadata {
        peer_id: PeerId,
        response: oneshot::Sender<Option<std::collections::HashMap<String, String>>>,
    },

    // NEW: Connection Management Commands
    
    /// Get all active connections
    GetAllConnections {
        response: oneshot::Sender<Vec<ConnectionInfo>>,
    },
    
    /// Get information about a specific peer
    GetPeerInfo {
        peer_id: PeerId,
        response: oneshot::Sender<Option<PeerInfo>>,
    },
    
    /// Get all connected peers
    GetConnectedPeers {
        response: oneshot::Sender<Vec<PeerInfo>>,
    },
    
    /// Get information about a specific connection
    GetConnectionInfo {
        connection_id: ConnectionId,
        response: oneshot::Sender<Option<ConnectionInfo>>,
    },
    
    /// Get overall network state
    GetNetworkState {
        response: oneshot::Sender<NetworkState>,
    },
    
    /// Disconnect a specific connection
    DisconnectConnection {
        connection_id: ConnectionId,
        response: oneshot::Sender<Result<(), String>>,
    },
    
    /// Disconnect all connections (shutdown)
    DisconnectAll {
        response: oneshot::Sender<Result<(), String>>,
    },

    // Core system commands
    Shutdown,

    // Nested XRoutes commands
    XRoutes(XRoutesCommand),
}

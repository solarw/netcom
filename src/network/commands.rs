use libp2p::{Multiaddr, PeerId, swarm::ConnectionId};
use std::error::Error;
use tokio::sync::oneshot;
use super::xauth::definitions::AuthResult;

// Add this to the NetworkCommand enum in commands.rs
#[derive(Debug)]
pub enum NetworkCommand {
    // MDNS commands
    EnableMdns,
    DisableMdns,

    // KAD commands
    EnableKad,
    DisableKad,

    // Connection commands
    OpenListenPort {
        port: u16,
        response: oneshot::Sender<Result<Multiaddr, Box<dyn Error + Send + Sync>>>,
    },
    Connect {
        addr: Multiaddr,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    Disconnect {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },

    // Status commands
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

    // Shutdown command
    Shutdown,

    GetKadKnownPeers {
        response: oneshot::Sender<Vec<(PeerId, Vec<Multiaddr>)>>,
    },

    GetPeerAddresses {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<Multiaddr>>,
    },
    
    // Initiate a network search for a peer's addresses
    FindPeerAddresses {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    // Check if a peer is authenticated
    IsPeerAuthenticated {
        peer_id: PeerId,
        response: oneshot::Sender<bool>,
    },
    
    // NEW: Submit PoR verification result
    SubmitPorVerification {
        connection_id: ConnectionId,
        result: AuthResult,
    },
}
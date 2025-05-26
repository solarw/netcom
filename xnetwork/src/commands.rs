// src/commands.rs

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};
use std::error::Error;
use tokio::sync::oneshot;
use xauth::definitions::AuthResult;
use xstream::xstream::XStream;

// Import XRoutes commands
use crate::xroutes::XRoutesCommand;

#[derive(Debug)]
pub enum NetworkCommand {
    // Core connection commands
    OpenListenPort {
        host: String,
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

    // Core stream commands
    OpenStream {
        peer_id: PeerId,
        connection_id: Option<ConnectionId>,
        response: oneshot::Sender<Result<XStream, String>>,
    },

    // Core status commands
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

    // Core system commands
    Shutdown,

    // Nested XRoutes commands - all XRoutes functionality is handled through this
    XRoutes(XRoutesCommand),
}
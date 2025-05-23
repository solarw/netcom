// src/xroutes/commands.rs

use libp2p::{Multiaddr, PeerId};
use std::error::Error;
use tokio::sync::oneshot;

use super::xroute::XRouteRole;
use super::types::{BootstrapNodeInfo, BootstrapError};

#[derive(Debug)]
pub enum XRoutesCommand {
    // mDNS commands
    EnableMdns,
    DisableMdns,
    
    // Kademlia commands
    EnableKad,
    DisableKad,
    BootstrapKad {
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    // Peer discovery commands
    GetKadKnownPeers {
        response: oneshot::Sender<Vec<(PeerId, Vec<Multiaddr>)>>,
    },
    GetPeerAddresses {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<Multiaddr>>,
    },
    FindPeerAddresses {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    // XRoute role management
    SetRole {
        role: XRouteRole,
        response: oneshot::Sender<Result<(), String>>,
    },
    GetRole {
        response: oneshot::Sender<XRouteRole>,
    },
    
    // Bootstrap connection - NEW
    ConnectToBootstrap {
        addr: Multiaddr,
        timeout_secs: Option<u64>,
        response: oneshot::Sender<Result<BootstrapNodeInfo, BootstrapError>>,
    },
}
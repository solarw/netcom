// src/xroutes/commands.rs

use libp2p::{Multiaddr, PeerId};
use std::error::Error;
use tokio::sync::oneshot;

use super::discovery::commands::DiscoveryCommand;
use super::types::{BootstrapError, BootstrapNodeInfo};
use super::xroute::XRouteRole;

#[derive(Debug)]
pub enum XRoutesCommand {
    // mDNS commands now in discovery
    DiscoveryCommand(DiscoveryCommand),

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

    // NEW: Advanced peer address finding with individual timeouts
    FindPeerAddressesAdvanced {
        peer_id: PeerId,
        timeout_secs: i32, // 0 = local only, >0 = timeout in seconds, -1 = infinite
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    },

    // NEW: Cancel active search for a peer
    CancelPeerSearch {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), String>>,
    },

    // NEW: Get active searches info
    GetActiveSearches {
        response: oneshot::Sender<Vec<(PeerId, usize, std::time::Duration)>>, // (peer_id, waiters_count, duration)
    },

    // XRoute role management
    SetRole {
        role: XRouteRole,
        response: oneshot::Sender<Result<(), String>>,
    },
    GetRole {
        response: oneshot::Sender<XRouteRole>,
    },

    // Bootstrap connection
    ConnectToBootstrap {
        addr: Multiaddr,
        timeout_secs: Option<u64>,
        response: oneshot::Sender<Result<BootstrapNodeInfo, BootstrapError>>,
    },
}

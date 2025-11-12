//! Commands for XRoutes behaviour

use libp2p::{PeerId, Multiaddr};
use super::types::XRoutesStatus;

/// Commands for controlling XRoutes behaviours
#[derive(Debug)]
pub enum XRoutesCommand {
    /// Enable identify behaviour
    EnableIdentify {
        /// Response channel for enable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Disable identify behaviour
    DisableIdentify {
        /// Response channel for disable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Enable mDNS discovery
    EnableMdns {
        /// Response channel for enable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Disable mDNS discovery
    DisableMdns {
        /// Response channel for disable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Enable Kademlia DHT discovery
    EnableKad {
        /// Response channel for enable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Disable Kademlia DHT discovery
    DisableKad {
        /// Response channel for disable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get current status of all behaviours
    GetStatus {
        /// Response channel for status
        response: tokio::sync::oneshot::Sender<XRoutesStatus>,
    },
    /// Bootstrap to a peer for Kademlia DHT
    BootstrapToPeer {
        /// Peer ID to bootstrap to
        peer_id: PeerId,
        /// Addresses of the bootstrap peer
        addresses: Vec<Multiaddr>,
        /// Response channel for bootstrap completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Find a peer through Kademlia DHT
    FindPeer {
        /// Peer ID to find
        peer_id: PeerId,
        /// Response channel with found addresses
        response: tokio::sync::oneshot::Sender<Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get closest peers through Kademlia DHT
    GetClosestPeers {
        /// Peer ID to search for
        peer_id: PeerId,
        /// Response channel with closest peers
        response: tokio::sync::oneshot::Sender<Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Find peer addresses with automatic search and timeout
    FindPeerAddresses {
        /// Peer ID to find
        peer_id: PeerId,
        /// Timeout for the search operation
        timeout: std::time::Duration,
        /// Response channel with found addresses
        response: tokio::sync::oneshot::Sender<Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>>>,
    },
}

//! Commands for XRoutes behaviour

use libp2p::{PeerId, Multiaddr};
use std::time::SystemTime;
use super::types::XRoutesStatus;

/// Status information for mDNS cache
#[derive(Debug, Clone)]
pub struct MdnsCacheStatus {
    /// Total number of peers in cache
    pub total_peers: usize,
    /// Current cache size (number of entries)
    pub cache_size: usize,
    /// When the cache was last updated
    pub last_update: SystemTime,
    /// Default TTL for cache entries in seconds
    pub ttl_seconds: u64,
}

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
    /// Get all peers from mDNS cache
    GetMdnsPeers {
        /// Response channel with all mDNS peers and their addresses
        response: tokio::sync::oneshot::Sender<Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Find a specific peer in mDNS cache
    FindMdnsPeer {
        /// Peer ID to find
        peer_id: PeerId,
        /// Response channel with found addresses
        response: tokio::sync::oneshot::Sender<Result<Option<Vec<Multiaddr>>, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Get mDNS cache status
    GetMdnsCacheStatus {
        /// Response channel with cache status
        response: tokio::sync::oneshot::Sender<Result<MdnsCacheStatus, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Clear mDNS cache
    ClearMdnsCache {
        /// Response channel with number of cleared entries
        response: tokio::sync::oneshot::Sender<Result<usize, Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Enable mDNS with custom TTL
    EnableMdnsWithTtl {
        /// Custom TTL in seconds
        ttl_seconds: u64,
        /// Response channel for enable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
    /// Enable relay server
    EnableRelayServer {
        /// Response channel for enable completion
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    },
}

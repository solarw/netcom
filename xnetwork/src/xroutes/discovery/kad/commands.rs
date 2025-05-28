// ./xroutes/discovery/kad/commands.rs

use libp2p::{Multiaddr, PeerId, kad};
use std::time::Duration;
use tokio::sync::oneshot;
use crate::xroutes::xroute::XRouteRole;

#[derive(Debug)]
pub enum KadCommand {
    // ==========================================
    // BASIC KADEMLIA COMMANDS
    // ==========================================
    
    /// Enable Kademlia DHT
    EnableKad,
    
    /// Disable Kademlia DHT
    DisableKad,
    
    /// Bootstrap the DHT
    Bootstrap,
    
    /// Add an address for a peer to the routing table
    AddAddress {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    
    /// Remove an address for a peer from the routing table
    RemoveAddress {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    
    /// Start searching for a peer (basic)
    FindPeer {
        peer_id: PeerId,
    },
    
    /// Set Kademlia mode (Server/Client)
    SetMode {
        mode: kad::Mode,
    },

    /// Get known peers from routing table
    GetKnownPeers {
        response: oneshot::Sender<Vec<(PeerId, Vec<Multiaddr>)>>,
    },

    /// Get addresses for specific peer from local routing table
    GetPeerAddresses {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<Multiaddr>>,
    },

    /// Get Kademlia statistics
    GetKadStats {
        response: oneshot::Sender<KadStats>,
    },

    // ==========================================
    // ADVANCED SEARCH COMMANDS
    // ==========================================
    
    /// Find peer addresses with advanced timeout control
    /// 
    /// # Arguments
    /// * `peer_id` - The peer to find addresses for
    /// * `timeout_secs` - Timeout behavior:
    ///   - `0` - Check local tables only, no DHT search
    ///   - `>0` - Search with specified timeout in seconds
    ///   - `-1` - Infinite search until explicitly cancelled
    FindPeerAddressesAdvanced {
        peer_id: PeerId,
        timeout_secs: i32,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    },

    /// Cancel active search for a specific peer
    CancelPeerSearch {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), String>>,
    },

    /// Cancel all active searches
    CancelAllSearches {
        reason: String,
        response: oneshot::Sender<()>,
    },

    /// Get information about active searches
    /// Returns list of (peer_id, waiters_count, search_duration)
    GetActiveSearches {
        response: oneshot::Sender<Vec<(PeerId, usize, Duration)>>,
    },

    /// Get search handler statistics
    GetSearchStats {
        response: oneshot::Sender<SearchHandlerStats>,
    },

    /// Cleanup timed out waiters (typically called periodically)
    CleanupTimedOutWaiters,

    // ==========================================
    // CONVENIENCE/COMPATIBILITY COMMANDS
    // ==========================================

    /// Find peer addresses with timeout (convenience command)
    FindPeerAddressesWithTimeout {
        peer_id: PeerId,
        timeout_secs: u32,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    },

    /// Find peer addresses from local tables only (convenience command)
    FindPeerAddressesLocalOnly {
        peer_id: PeerId,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    },

    /// Find peer addresses with infinite timeout (convenience command)
    FindPeerAddressesInfinite {
        peer_id: PeerId,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    },

    /// Connect to bootstrap node and verify it
    ConnectToBootstrapNode {
        addr: Multiaddr,
        timeout_secs: Option<u64>,
        response: oneshot::Sender<Result<BootstrapNodeInfo, BootstrapError>>,
    },
}

/// Statistics about the Kademlia routing table
#[derive(Debug, Clone)]
pub struct KadStats {
    pub total_peers: usize,
    pub active_buckets: usize,
    pub total_buckets: usize,
}

impl KadStats {
    /// Calculate the average peers per active bucket
    pub fn avg_peers_per_bucket(&self) -> f32 {
        if self.active_buckets > 0 {
            self.total_peers as f32 / self.active_buckets as f32
        } else {
            0.0
        }
    }

    /// Calculate the fill ratio of the routing table
    pub fn fill_ratio(&self) -> f32 {
        if self.total_buckets > 0 {
            self.active_buckets as f32 / self.total_buckets as f32
        } else {
            0.0
        }
    }
}

/// Statistics for the search handler
#[derive(Debug, Clone)]
pub struct SearchHandlerStats {
    pub active_searches: usize,
    pub total_waiters: usize,
    pub cached_peers: usize,
    pub next_waiter_id: u64,
}

// ==========================================
// BOOTSTRAP CONNECTION TYPES
// ==========================================

/// Information about a bootstrap node
#[derive(Debug, Clone)]
pub struct BootstrapNodeInfo {
    pub peer_id: PeerId,
    pub role: XRouteRole,
    pub protocols: Vec<String>,
    pub agent_version: String,
}

/// Errors that can occur during bootstrap connection
#[derive(Debug)]
pub enum BootstrapError {
    ConnectionFailed(String),
    ConnectionTimeout,
    InvalidAddress(String),
    RoleCheckFailed(String),
    NotABootstrapServer(XRouteRole),
    KadNotEnabled,
}

impl std::fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootstrapError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            BootstrapError::ConnectionTimeout => write!(f, "Connection timed out"),
            BootstrapError::InvalidAddress(msg) => write!(f, "Invalid address: {}", msg),
            BootstrapError::RoleCheckFailed(msg) => write!(f, "Role check failed: {}", msg),
            BootstrapError::NotABootstrapServer(role) => write!(f, "Not a bootstrap server, found role: {:?}", role),
            BootstrapError::KadNotEnabled => write!(f, "Kademlia not enabled"),
        }
    }
}

impl std::error::Error for BootstrapError {}
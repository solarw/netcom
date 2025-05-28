// ./xroutes/discovery/kad/events.rs

use libp2p::{Multiaddr, PeerId, kad};

#[derive(Debug, Clone)]
pub enum KadEvent {
    /// Kademlia was enabled
    KadEnabled,
    
    /// Kademlia was disabled
    KadDisabled,
    
    /// Bootstrap completed
    BootstrapCompleted {
        bootstrap_peers: Vec<PeerId>,
    },
    
    /// Bootstrap failed
    BootstrapFailed {
        error: String,
    },
    
    /// Routing table updated with new peer information
    RoutingUpdated {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    
    /// Peer discovered through DHT query
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    
    /// DHT query completed
    QueryCompleted {
        query_id: kad::QueryId,
        result: QueryResult,
    },
    
    /// DHT query failed
    QueryFailed {
        query_id: kad::QueryId,
        error: String,
    },
    
    /// Address added to routing table
    AddressAdded {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    
    /// Address removed from routing table
    AddressRemoved {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    
    /// Kademlia mode changed
    ModeChanged {
        old_mode: Option<kad::Mode>,
        new_mode: kad::Mode,
    },

    // ==========================================
    // ADVANCED SEARCH EVENTS
    // ==========================================
    
    /// Advanced search started for a peer
    SearchStarted {
        peer_id: PeerId,
        search_id: u64,
        timeout_secs: i32,
    },
    
    /// Advanced search completed successfully
    SearchCompleted {
        peer_id: PeerId,
        search_id: u64,
        addresses: Vec<Multiaddr>,
        duration: std::time::Duration,
    },
    
    /// Advanced search failed
    SearchFailed {
        peer_id: PeerId,
        search_id: u64,
        error: String,
        duration: std::time::Duration,
    },
    
    /// Advanced search timed out
    SearchTimedOut {
        peer_id: PeerId,
        search_id: u64,
        duration: std::time::Duration,
    },
    
    /// Advanced search was cancelled
    SearchCancelled {
        peer_id: PeerId,
        search_id: u64,
        reason: String,
    },
    
    /// Multiple searches were cancelled
    AllSearchesCancelled {
        count: usize,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub enum QueryResult {
    /// Query found peers
    FoundPeers(Vec<(PeerId, Vec<Multiaddr>)>),
    
    /// Query found no peers
    NoPeersFound,
    
    /// Query timed out
    Timeout,
    
    /// Other query result
    Other(String),
}
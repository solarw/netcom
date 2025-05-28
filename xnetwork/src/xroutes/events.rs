// src/xroutes/events.rs

use libp2p::{Multiaddr, PeerId};

use super::{discovery::events::DiscoveryEvent, xroute::XRouteRole};

#[derive(Debug, Clone)]
pub enum XRoutesEvent {
    // mDNS events
    MdnsEnabled,
    MdnsDisabled,
    MdnsPeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    MdnsPeerExpired {
        peer_id: PeerId,
    },

    DiscoveryEvent (DiscoveryEvent),
    
    // Kademlia events
    KadEnabled,
    KadDisabled,
    KadAddressAdded {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    KadRoutingUpdated {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    KadBootstrapCompleted {
        bootstrap_peers: Vec<PeerId>,
    },
    KadQueryCompleted {
        query_id: String,
        result: String,
    },
    KadPeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    
    // XRoute role events
    RoleChanged {
        old_role: XRouteRole,
        new_role: XRouteRole,
    },
    PeerRoleDetected {
        peer_id: PeerId,
        role: XRouteRole,
    },
}
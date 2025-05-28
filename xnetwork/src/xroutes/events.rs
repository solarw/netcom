// src/xroutes/events.rs

use libp2p::{Multiaddr, PeerId};

use super::{discovery::events::DiscoveryEvent, connectivity::events::ConnectivityEvent, xroute::XRouteRole};

#[derive(Debug, Clone)]
pub enum XRoutesEvent {
    // Discovery events (includes mDNS and Kademlia)
    DiscoveryEvent(DiscoveryEvent),
    
    // Connectivity events (includes relay)
    ConnectivityEvent(ConnectivityEvent),
    
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

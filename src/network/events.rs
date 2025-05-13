// Файл: ./src/network/events.rs

use std::sync::Arc;

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId, StreamProtocol};
use super::{xauth::events::PorAuthEvent, xstream::xstream::XStream};

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerConnected {
        //when peer conencts for the first time
        peer_id: PeerId,
    },
    PeerDisconnected {
        // when all peer connections closed
        peer_id: PeerId,
    },
    ConnectionError {
        peer_id: Option<PeerId>,
        error: String,
    },

    // Connection events
    ConnectionOpened {
        peer_id: PeerId,
        addr: Multiaddr,
        connection_id: ConnectionId,
        protocols: Vec<StreamProtocol>,  
    },
    ConnectionClosed {
        peer_id: PeerId,
        addr: Multiaddr,
        connection_id: ConnectionId,
    },
    ListeningOnAddress {
        addr: Multiaddr,
        full_addr: Option<Multiaddr>,
    },
    StopListeningOnAddress {
        addr: Multiaddr,
    },
    
    // Discovery events
    MdnsIsOn {},
    MdnsIsOff {},
    
    // Kademlia DHT events
    KadAddressAdded {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    KadRoutingUpdated {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    
    // Authentication event wrapper
    AuthEvent {
        event: PorAuthEvent,
    },

    IncomingStream {
        stream: Arc<XStream>
    },

}
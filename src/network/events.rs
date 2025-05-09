use std::sync::Arc;

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId, StreamProtocol};
use super::{xauth::events::PorAuthEvent, xstream::manager::XStream};

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
    
    // Новый вариант для таймаута потока
    #[cfg(feature = "stream_timeout_event")]
    StreamTimeoutEvent {
        id: u128,
        is_main: bool,
        peer_id: Option<PeerId>,
    },
}
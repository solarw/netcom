// src/events.rs

use std::sync::Arc;

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId, StreamProtocol};
use xauth::events::PorAuthEvent;
use xstream::xstream::XStream;

// Import XRoutes events
use crate::xroutes::XRoutesEvent;

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    // Core peer connection events
    PeerConnected {
        // when peer connects for the first time
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

    // Core connection events
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
    
    // Core listening events
    ListeningOnAddress {
        addr: Multiaddr,
        full_addr: Option<Multiaddr>,
    },
    StopListeningOnAddress {
        addr: Multiaddr,
    },

    // Core authentication events
    AuthEvent {
        event: PorAuthEvent,
    },

    // Core stream events
    IncomingStream {
        stream: Arc<XStream>,
    },

    // Nested XRoutes events
    XRoutes(XRoutesEvent),
}
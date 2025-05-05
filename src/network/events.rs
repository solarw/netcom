use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};

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

    // new events
    ConnectionOpened {
        peer_id: PeerId,
        addr: Multiaddr,
        connection_id: ConnectionId,
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
    MdnsIsOn {},
    MdnsIsOff {},

}

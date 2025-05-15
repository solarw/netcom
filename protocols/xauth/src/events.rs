use std::collections::HashMap;

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};

use super::{definitions::AuthDirection, por::por::ProofOfRepresentation};

// Events emitted by the behaviour
#[derive(Debug, Clone)]
pub enum PorAuthEvent {
    // Authentication succeeded in both directions
    MutualAuthSuccess {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        metadata: HashMap<String, String>,
    },
    // We authenticated the remote peer
    OutboundAuthSuccess {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        metadata: HashMap<String, String>,
    },
    // Remote peer authenticated us
    InboundAuthSuccess {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
    },
    // We rejected the remote peer's authentication
    OutboundAuthFailure {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        reason: String,
    },
    // Remote peer rejected our authentication
    InboundAuthFailure {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        reason: String,
    },
    // Authentication timeout
    AuthTimeout {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        direction: AuthDirection,
    },
    // PoR verification needed
    VerifyPorRequest {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        por: ProofOfRepresentation,
        metadata: HashMap<String, String>,
    },
}

//! Node events for XNetwork2
//!
//! Cloneable events that are sent to developers through event channels

use libp2p::{Multiaddr, PeerId, swarm::ConnectionId};
use tokio::sync::oneshot;
use xstream::events::{InboundUpgradeDecision, StreamOpenDecisionSender};
use xstream::types::XStreamID;
use xstream::xstream::XStream;

/// Node events that are sent to developers
#[derive(Debug, Clone)]
pub enum NodeEvent {
    // Сетевые события
    /// Connection established with peer
    ConnectionEstablished { 
        peer_id: PeerId,
        connection_id: ConnectionId 
    },
    /// Connection closed with peer
    ConnectionClosed { 
        peer_id: PeerId,
        connection_id: ConnectionId 
    },
    /// New listener address added
    NewListenAddr { address: Multiaddr },
    /// Listener address removed
    ExpiredListenAddr { address: Multiaddr },

    // Аутентификация события
    /// Mutual authentication successfully completed
    PeerMutualAuthSuccess { 
        peer_id: PeerId,
        connection_id: ConnectionId 
    },
    /// Outbound authentication successfully completed
    PeerOutboundAuthSuccess { 
        peer_id: PeerId,
        connection_id: ConnectionId 
    },
    /// Inbound authentication successfully completed
    PeerInboundAuthSuccess { 
        peer_id: PeerId,
        connection_id: ConnectionId 
    },
    /// PoR verification requested
    VerifyPorRequest {
        peer_id: PeerId,
        connection_id: String,
        por: Vec<u8>,
        metadata: std::collections::HashMap<String, String>,
    },

    // XStream события
    /// Входящий XStream поток
    XStreamIncoming { stream: XStream },
    /// Исходящий XStream поток установлен
    XStreamEstablished {
        peer_id: PeerId,
        stream_id: XStreamID,
    },
    /// Ошибка при работе с XStream
    XStreamError {
        peer_id: PeerId,
        stream_id: Option<XStreamID>,
        error: String,
    },
    /// XStream поток закрыт
    XStreamClosed {
        peer_id: PeerId,
        stream_id: XStreamID,
    },
    /// Запрос на принятие решения о входящем потоке XStream
    XStreamIncomingStreamRequest {
        peer_id: PeerId,
        connection_id: ConnectionId,
        decision_sender: StreamOpenDecisionSender,
    },

    // Identify события
    /// Identify information received from peer
    IdentifyReceived { 
        peer_id: PeerId, 
        addresses: Vec<Multiaddr> 
    },
    /// Identify information sent to peer
    IdentifySent { 
        peer_id: PeerId 
    },
    /// Identify error occurred
    IdentifyError { 
        peer_id: PeerId, 
        error: String 
    },

    // Kademlia события
    /// Kademlia discovered a new peer through DHT
    KademliaPeerDiscovered { 
        peer_id: PeerId, 
        addresses: Vec<Multiaddr> 
    },
    /// Kademlia bootstrap completed
    KademliaBootstrapCompleted,
    /// Kademlia routing table updated
    KademliaRoutingUpdated { 
        peer_id: PeerId 
    },
}

impl NodeEvent {
    /// Get a descriptive name for the event
    pub fn name(&self) -> &'static str {
        match self {
            NodeEvent::ConnectionEstablished { .. } => "ConnectionEstablished",
            NodeEvent::ConnectionClosed { .. } => "ConnectionClosed",
            NodeEvent::NewListenAddr { .. } => "NewListenAddr",
            NodeEvent::ExpiredListenAddr { .. } => "ExpiredListenAddr",
            NodeEvent::PeerMutualAuthSuccess { .. } => "PeerMutualAuthSuccess",
            NodeEvent::PeerOutboundAuthSuccess { .. } => "PeerOutboundAuthSuccess",
            NodeEvent::PeerInboundAuthSuccess { .. } => "PeerInboundAuthSuccess",
            NodeEvent::VerifyPorRequest { .. } => "VerifyPorRequest",
            NodeEvent::XStreamIncoming { .. } => "XStreamIncoming",
            NodeEvent::XStreamEstablished { .. } => "XStreamEstablished",
            NodeEvent::XStreamError { .. } => "XStreamError",
            NodeEvent::XStreamClosed { .. } => "XStreamClosed",
            NodeEvent::XStreamIncomingStreamRequest { .. } => "XStreamIncomingStreamRequest",
            NodeEvent::IdentifyReceived { .. } => "IdentifyReceived",
            NodeEvent::IdentifySent { .. } => "IdentifySent",
            NodeEvent::IdentifyError { .. } => "IdentifyError",
            NodeEvent::KademliaPeerDiscovered { .. } => "KademliaPeerDiscovered",
            NodeEvent::KademliaBootstrapCompleted { .. } => "KademliaBootstrapCompleted",
            NodeEvent::KademliaRoutingUpdated { .. } => "KademliaRoutingUpdated",
        }
    }

    /// Check if this is a network-related event
    pub fn is_network_event(&self) -> bool {
        matches!(
            self,
            NodeEvent::ConnectionEstablished { .. }
                | NodeEvent::ConnectionClosed { .. }
                | NodeEvent::NewListenAddr { .. }
                | NodeEvent::ExpiredListenAddr { .. }
        )
    }

    /// Check if this is an authentication-related event
    pub fn is_auth_event(&self) -> bool {
        matches!(
            self,
            NodeEvent::PeerMutualAuthSuccess { .. }
                | NodeEvent::PeerOutboundAuthSuccess { .. }
                | NodeEvent::PeerInboundAuthSuccess { .. }
                | NodeEvent::VerifyPorRequest { .. }
        )
    }

    /// Check if this is a stream-related event
    pub fn is_stream_event(&self) -> bool {
        matches!(
            self,
            NodeEvent::XStreamIncoming { .. }
                | NodeEvent::XStreamEstablished { .. }
                | NodeEvent::XStreamError { .. }
                | NodeEvent::XStreamClosed { .. }
                | NodeEvent::XStreamIncomingStreamRequest { .. }
        )
    }
}

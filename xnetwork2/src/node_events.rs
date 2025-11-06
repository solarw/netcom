//! Node events for XNetwork2
//! 
//! Cloneable events that are sent to developers through event channels

use libp2p::{Multiaddr, PeerId};

/// Node events that are sent to developers
#[derive(Clone, Debug)]
pub enum NodeEvent {
    // Сетевые события
    /// Node started listening on an address
    ListeningOn { address: Multiaddr },
    /// Peer connected to the node
    PeerConnected { peer_id: PeerId },
    /// Peer disconnected from the node
    PeerDisconnected { peer_id: PeerId },
    /// Connection established with peer
    ConnectionEstablished { peer_id: PeerId },
    /// Connection closed with peer
    ConnectionClosed { peer_id: PeerId },
    /// New listener address added
    NewListenAddr { address: Multiaddr },
    /// Listener address removed
    ExpiredListenAddr { address: Multiaddr },
    
    // Аутентификация события
    /// Peer successfully authenticated
    PeerAuthenticated { peer_id: PeerId },
    /// Peer authentication failed
    AuthenticationFailed { peer_id: PeerId },
    
    // XStream события
    /// New stream opened with peer
    StreamOpened { peer_id: PeerId, stream_id: String },
    /// Stream closed with peer
    StreamClosed { peer_id: PeerId, stream_id: String },
    /// Data received on stream
    DataReceived { peer_id: PeerId, stream_id: String, data: Vec<u8> },
    
    // Системные события
    /// Node started successfully
    NodeStarted,
    /// Node stopped
    NodeStopped,
    /// Error occurred
    Error { message: String },
    
    // Поведенческие события
    /// Identify information received
    IdentifyReceived { peer_id: PeerId, info: String },
    /// Ping response received
    PingResponse { peer_id: PeerId, rtt: std::time::Duration },
}

impl NodeEvent {
    /// Get a descriptive name for the event
    pub fn name(&self) -> &'static str {
        match self {
            NodeEvent::ListeningOn { .. } => "ListeningOn",
            NodeEvent::PeerConnected { .. } => "PeerConnected",
            NodeEvent::PeerDisconnected { .. } => "PeerDisconnected",
            NodeEvent::ConnectionEstablished { .. } => "ConnectionEstablished",
            NodeEvent::ConnectionClosed { .. } => "ConnectionClosed",
            NodeEvent::NewListenAddr { .. } => "NewListenAddr",
            NodeEvent::ExpiredListenAddr { .. } => "ExpiredListenAddr",
            NodeEvent::PeerAuthenticated { .. } => "PeerAuthenticated",
            NodeEvent::AuthenticationFailed { .. } => "AuthenticationFailed",
            NodeEvent::StreamOpened { .. } => "StreamOpened",
            NodeEvent::StreamClosed { .. } => "StreamClosed",
            NodeEvent::DataReceived { .. } => "DataReceived",
            NodeEvent::NodeStarted => "NodeStarted",
            NodeEvent::NodeStopped => "NodeStopped",
            NodeEvent::Error { .. } => "Error",
            NodeEvent::IdentifyReceived { .. } => "IdentifyReceived",
            NodeEvent::PingResponse { .. } => "PingResponse",
        }
    }
    
    /// Check if this is a network-related event
    pub fn is_network_event(&self) -> bool {
        matches!(
            self,
            NodeEvent::ListeningOn { .. }
                | NodeEvent::PeerConnected { .. }
                | NodeEvent::PeerDisconnected { .. }
                | NodeEvent::ConnectionEstablished { .. }
                | NodeEvent::ConnectionClosed { .. }
                | NodeEvent::NewListenAddr { .. }
                | NodeEvent::ExpiredListenAddr { .. }
        )
    }
    
    /// Check if this is an authentication-related event
    pub fn is_auth_event(&self) -> bool {
        matches!(
            self,
            NodeEvent::PeerAuthenticated { .. } | NodeEvent::AuthenticationFailed { .. }
        )
    }
    
    /// Check if this is a stream-related event
    pub fn is_stream_event(&self) -> bool {
        matches!(
            self,
            NodeEvent::StreamOpened { .. }
                | NodeEvent::StreamClosed { .. }
                | NodeEvent::DataReceived { .. }
        )
    }
}

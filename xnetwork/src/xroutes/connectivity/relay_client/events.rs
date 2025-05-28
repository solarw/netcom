use libp2p::{Multiaddr, PeerId};

#[derive(Debug, Clone)]
pub enum RelayClientEvent {
    /// Successfully connected via relay
    RelayConnectionEstablished {
        relay_addr: Multiaddr,
        target_peer_id: PeerId,
    },
    
    /// Failed to connect via relay
    RelayConnectionFailed {
        relay_addr: Multiaddr,
        target_peer_id: PeerId,
        error: String,
    },
    
    /// Relay reservation made successfully
    ReservationMade {
        relay_peer_id: PeerId,
        relay_addr: Multiaddr,
    },
    
    /// Relay reservation failed
    ReservationFailed {
        relay_peer_id: PeerId,
        relay_addr: Multiaddr,
        error: String,
    },
    
    /// Relay reservation expired
    ReservationExpired {
        relay_peer_id: PeerId,
    },
    
    /// Started listening via relay
    ListeningViaRelay {
        relay_addr: Multiaddr,
        listen_addr: Multiaddr,
    },
    
    /// Stopped listening via relay
    StoppedListeningViaRelay {
        relay_addr: Multiaddr,
    },
    
    /// Incoming connection via relay
    IncomingConnectionViaRelay {
        relay_peer_id: PeerId,
        remote_peer_id: PeerId,
    },
}

#[derive(Debug, Clone, Default)]
pub struct RelayClientStats {
    /// Number of successful outbound relay connections
    pub successful_outbound_connections: usize,
    /// Number of failed outbound relay connections
    pub failed_outbound_connections: usize,
    /// Number of active reservations
    pub active_reservations: usize,
    /// Number of total reservations made
    pub total_reservations_made: usize,
    /// Number of incoming connections via relay
    pub incoming_connections_via_relay: usize,
}

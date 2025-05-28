use libp2p::{Multiaddr, PeerId};

#[derive(Debug, Clone)]
pub enum RelayServerEvent {
    /// Relay server started successfully
    ServerStarted {
        listen_addr: Multiaddr,
    },
    
    /// Relay server stopped
    ServerStopped,
    
    /// A client made a reservation
    ReservationMade {
        client_peer_id: PeerId,
    },
    
    /// A client's reservation expired
    ReservationExpired {
        client_peer_id: PeerId,
    },
    
    /// A circuit was established between two clients
    CircuitEstablished {
        src_peer_id: PeerId,
        dst_peer_id: PeerId,
    },
    
    /// A circuit establishment failed
    CircuitFailed {
        src_peer_id: PeerId,
        dst_peer_id: PeerId,
        error: String,
    },
    
    /// A circuit was closed
    CircuitClosed {
        src_peer_id: PeerId,
        dst_peer_id: PeerId,
    },
    
    /// Server configuration changed
    ConfigurationChanged {
        max_reservations: usize,
        max_circuits: usize,
    },
}

#[derive(Debug, Clone, Default)]
pub struct RelayServerStats {
    /// Number of active reservations
    pub active_reservations: usize,
    /// Total number of reservations handled
    pub total_reservations_handled: usize,
    /// Number of active circuits
    pub active_circuits: usize,
    /// Total number of circuits handled
    pub total_circuits_handled: usize,
    /// Number of failed circuit requests
    pub failed_circuit_requests: usize,
    /// Server uptime in seconds
    pub uptime_seconds: u64,
}

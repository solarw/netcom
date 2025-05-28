use libp2p::Multiaddr;
use tokio::sync::oneshot;

use super::events::RelayServerStats;

#[derive(Debug)]
pub enum RelayServerCommand {
    /// Start the relay server
    Start {
        listen_addr: Option<Multiaddr>,
        response: oneshot::Sender<Result<Multiaddr, String>>,
    },
    
    /// Stop the relay server
    Stop {
        response: oneshot::Sender<Result<(), String>>,
    },
    
    /// Get server status (running or stopped)
    GetStatus {
        response: oneshot::Sender<bool>,
    },
    
    /// Get relay server statistics
    GetStats {
        response: oneshot::Sender<RelayServerStats>,
    },
    
    /// Configure server limits
    Configure {
        max_reservations: Option<usize>,
        max_circuits: Option<usize>,
        response: oneshot::Sender<Result<(), String>>,
    },
    
    /// Get current server configuration
    GetConfiguration {
        response: oneshot::Sender<(usize, usize)>, // (max_reservations, max_circuits)
    },
    
    /// Force close a specific circuit
    CloseCircuit {
        src_peer_id: libp2p::PeerId,
        dst_peer_id: libp2p::PeerId,
        response: oneshot::Sender<Result<(), String>>,
    },
    
    /// List all active reservations
    ListReservations {
        response: oneshot::Sender<Vec<libp2p::PeerId>>,
    },
    
    /// List all active circuits
    ListCircuits {
        response: oneshot::Sender<Vec<(libp2p::PeerId, libp2p::PeerId)>>,
    },
}

use libp2p::Multiaddr;
use tokio::sync::oneshot;

use super::events::RelayClientStats;

#[derive(Debug)]
pub enum RelayClientCommand {
    /// Listen for incoming connections via a specific relay server
    /// This will automatically handle reservation with the relay
    ListenViaRelay {
        relay_addr: Multiaddr,
        response: oneshot::Sender<Result<Multiaddr, String>>,
    },
    
    /// Stop listening via a specific relay server
    StopListeningViaRelay {
        relay_addr: Multiaddr,
        response: oneshot::Sender<Result<(), String>>,
    },
    
    /// Get relay client statistics
    GetStats {
        response: oneshot::Sender<RelayClientStats>,
    },
}

//! XAuth commands for XNetwork2

use libp2p::{PeerId, swarm::ConnectionId};
use tokio::sync::oneshot;

/// Commands for XAuth behaviour
#[derive(Debug)]
pub enum XAuthCommand {
    /// Start authentication with a peer (legacy - uses peer_id)
    StartAuth { peer_id: PeerId },
    /// Start authentication for specific connection
    StartAuthForConnection { 
        connection_id: ConnectionId,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>
    },
    /// Set authentication mode (automatic or manual)
    SetAutoAuthMode { 
        auto: bool,
        response: oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>
    },
    /// Approve authentication request
    ApproveAuth { peer_id: PeerId },
    /// Reject authentication request
    RejectAuth { peer_id: PeerId },
    /// Submit PoR verification result
    SubmitPorVerification { peer_id: PeerId, approved: bool },
}

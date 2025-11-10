//! XAuth commands for XNetwork2

use libp2p::PeerId;

/// Commands for XAuth behaviour
#[derive(Debug, Clone)]
pub enum XAuthCommand {
    /// Start authentication with a peer
    StartAuth { peer_id: PeerId },
    /// Approve authentication request
    ApproveAuth { peer_id: PeerId },
    /// Reject authentication request
    RejectAuth { peer_id: PeerId },
    /// Submit PoR verification result
    SubmitPorVerification { peer_id: PeerId, approved: bool },
}

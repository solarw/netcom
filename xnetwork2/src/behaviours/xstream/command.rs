//! XStream commands for XNetwork2

use libp2p::PeerId;

/// Commands for XStream behaviour
#[derive(Debug, Clone)]
pub enum XStreamCommand {
    // Note: XStream automatically handles stream lifecycle
    // No manual commands needed for now
}

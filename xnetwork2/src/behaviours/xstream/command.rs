//! XStream commands for XNetwork2

use libp2p::PeerId;

/// Commands for XStream behaviour
#[derive(Debug, Clone)]
pub enum XStreamCommand {
    /// Open a stream to a peer
    OpenStream {
        peer_id: PeerId,
    },
    /// Send data through a stream
    SendData {
        peer_id: PeerId,
        data: Vec<u8>,
    },
    /// Close a stream
    CloseStream {
        peer_id: PeerId,
    },
}

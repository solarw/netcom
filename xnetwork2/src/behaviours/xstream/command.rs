//! XStream commands for XNetwork2

use libp2p::PeerId;
use tokio::sync::oneshot;
use xstream::xstream::XStream;

/// Commands for XStream behaviour
#[derive(Debug)]
pub enum XStreamCommand {
    /// Open a new XStream to the specified peer
    OpenStream {
        /// Peer ID to open stream to
        peer_id: PeerId,
        /// Response channel for the created XStream
        response: oneshot::Sender<Result<XStream, String>>,
    },
}

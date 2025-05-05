use libp2p::{Multiaddr, PeerId};
use std::error::Error;
use tokio::sync::oneshot;
#[derive(Debug)]
pub enum NetworkCommand {
    // MDNS commands
    EnableMdns,
    DisableMdns,

    // KAD commands
    EnableKad,
    DisableKad,


    // Connection commands
    OpenListenPort {
        port: u16,
        response: oneshot::Sender<Result<Multiaddr, Box<dyn Error + Send + Sync>>>,
    },
    Connect {
        addr: Multiaddr,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    Disconnect {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },

    // Status commands
    GetConnectionsForPeer {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<Multiaddr>>,
    },

    GetPeersConnected {
        peer_id: PeerId,
        response: oneshot::Sender<Vec<(PeerId, Vec<Multiaddr>)>>,
    },

    GetListenAddresses {
        response: oneshot::Sender<Vec<Multiaddr>>,
    },

    // Shutdown command
    Shutdown,
}

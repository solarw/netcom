use libp2p::{Multiaddr, PeerId};

#[derive(Debug, Clone)]
pub enum MdnsEvent {
    MdnsEnabled,
    MdnsDisabled,
    MdnsPeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    MdnsPeerExpired {
        peer_id: PeerId,
    },
}

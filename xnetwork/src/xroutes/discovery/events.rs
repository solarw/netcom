use libp2p::{Multiaddr, PeerId};


#[derive(Debug, Clone)]
pub enum DiscoverySource {
    Mdns,
    Kad,
}
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
        source: DiscoverySource
    },
    PeerExpired {
        peer_id: PeerId,
        source: DiscoverySource
    },
}
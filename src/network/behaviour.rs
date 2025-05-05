use libp2p::{
    identify, identity,
    kad::{self, BucketInserts},
    mdns, ping,
    swarm::NetworkBehaviour,
};

use super::xauth::behaviours::PorAuthBehaviour;

#[derive(NetworkBehaviour)]
pub struct NodeBehaviour {
    pub identify: identify::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub ping: ping::Behaviour,
}

pub fn make_behaviour(key: &identity::Keypair) -> NodeBehaviour {
    let mut kad_config = kad::Config::default();
    kad_config.set_query_timeout(std::time::Duration::from_secs(300)); // 5 minutes
    kad_config.disjoint_query_paths(true);

    // Most importantly - set bucket insertion to Manual
    kad_config.set_kbucket_inserts(BucketInserts::OnConnected);

    // Create the store
    let kad_store = kad::store::MemoryStore::new(key.public().to_peer_id());

    // Create the Kademlia behavior with the custom config
    let kad_behaviour =
        kad::Behaviour::with_config(key.public().to_peer_id(), kad_store, kad_config);
    // Set up mDNS discovery
    let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())
        .expect("Failed to create mDNS behavior");

    // Set up the Identify protocol
    let identify = identify::Behaviour::new(
        identify::Config::new("/ipfs/id/1.0.0".to_string(), key.public())
            .with_agent_version(format!("p2p-network/{}", env!("CARGO_PKG_VERSION"))),
    );

    // Set up ping behavior for connection keep-alive
    let ping = ping::Behaviour::new(ping::Config::default());

    // Set up Rendezvous client for peer discovery
    // Create the network behavior
    NodeBehaviour {
        identify,
        kad: kad_behaviour,
        mdns,
        ping,
    }
}

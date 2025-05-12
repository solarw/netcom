use libp2p::{
    identify, identity,
    kad::{self, BucketInserts},
    mdns, ping,
    swarm::NetworkBehaviour,
};

use libp2p_stream;

use super::xauth::behaviours::PorAuthBehaviour;
use super::xauth::por::por::ProofOfRepresentation;

#[derive(NetworkBehaviour)]
pub struct NodeBehaviour {
    pub identify: identify::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub ping: ping::Behaviour,
    pub por_auth: PorAuthBehaviour,
    pub stream: libp2p_stream::Behaviour,
}

use std::num::NonZeroUsize;
pub fn make_behaviour(key: &identity::Keypair, por: ProofOfRepresentation) -> NodeBehaviour {
    let mut kad_config = kad::Config::default();

    // Increase query parallelism for faster discovery
    kad_config.set_parallelism(NonZeroUsize::new(3).unwrap());

    // Set more aggressive timeouts
    kad_config.set_query_timeout(std::time::Duration::from_secs(30)); // 30 seconds instead of 60

    // Enable disjoint query paths for better robustness
    kad_config.disjoint_query_paths(true);

    // Set the bucket insertion strategy
    // Note: Manual is more controlled but can be restrictive
    // Try using 'Always' for better discovery if available in your libp2p version
    kad_config.set_kbucket_inserts(kad::BucketInserts::OnConnected);

    // Disable automatic bootstrap throttling for more frequent bootstrap
    kad_config.set_automatic_bootstrap_throttle(None);

    // Set a more aggressive replication factor
    kad_config.set_replication_factor(NonZeroUsize::new(3).unwrap());

    // Create the store with the peer's ID
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

    // Set up PoR authentication behavior with the provided PoR
    let por_auth = PorAuthBehaviour::new(por);

    // Set up stream behavior
    let stream = libp2p_stream::Behaviour::new();

    // Create the network behavior
    NodeBehaviour {
        identify,
        kad: kad_behaviour,
        mdns,
        ping,
        por_auth,
        stream,
    }
}

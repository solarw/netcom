use std::num::NonZeroUsize;

use libp2p::{
    identify, identity,
    kad::{self, BucketInserts},
    mdns, ping,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
};

use libp2p_stream;

use super::xauth::por::por::ProofOfRepresentation;
use super::{xauth::behaviours::PorAuthBehaviour, xstream::behaviour::XStreamNetworkBehaviour};

// In the NodeBehaviour struct, use Toggle for mdns
#[derive(NetworkBehaviour)]
pub struct NodeBehaviour {
    pub identify: identify::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    // Use Toggle for mdns
    pub mdns: Toggle<mdns::tokio::Behaviour>,
    pub ping: ping::Behaviour,
    pub por_auth: PorAuthBehaviour,
    pub xstream: XStreamNetworkBehaviour,
}

// Update the make_behaviour function to use Toggle
pub fn make_behaviour(
    key: &identity::Keypair,
    por: ProofOfRepresentation,
    enable_mdns: bool,
    kad_server_mode: bool, // Add parameter for server mode
) -> NodeBehaviour {
    let mut kad_config = kad::Config::default();

    // Configure Kademlia behavior based on mode
    if kad_server_mode {
        // Server mode settings - more aggressive for bootstrap nodes
        // Increase query parallelism for faster discovery
        kad_config.set_parallelism(NonZeroUsize::new(5).unwrap());

        // Set more aggressive timeouts
        kad_config.set_query_timeout(std::time::Duration::from_secs(20));

        // Enable disjoint query paths for better robustness
        kad_config.disjoint_query_paths(true);

        // Set the bucket insertion strategy to be more permissive
        kad_config.set_kbucket_inserts(kad::BucketInserts::OnConnected);

        // Disable automatic bootstrap throttling for more frequent bootstrap
        kad_config.set_automatic_bootstrap_throttle(None);

        // Set a more aggressive replication factor
        kad_config.set_replication_factor(NonZeroUsize::new(5).unwrap());
    } else {
        // Client mode settings - default with minor tweaks
        // Slightly increase parallelism
        kad_config.set_parallelism(NonZeroUsize::new(3).unwrap());

        // Set reasonable timeouts for clients
        kad_config.set_query_timeout(std::time::Duration::from_secs(30));
    }

    kad_config.set_kbucket_inserts(BucketInserts::Manual);

    // 2. Disable periodic bootstrap (set to None)
    kad_config.set_periodic_bootstrap_interval(None);

    // 3. Disable automatic bootstrap
    kad_config.set_automatic_bootstrap_throttle(None);

    // Create the store
    let kad_store = kad::store::MemoryStore::new(key.public().to_peer_id());
    // 1. Set bucket inserts to manual

    // Create the Kademlia behavior with the custom config
    let mut kad_behaviour =
        kad::Behaviour::with_config(key.public().to_peer_id(), kad_store, kad_config);

    if kad_server_mode {
        kad_behaviour.set_mode(Some(kad::Mode::Server));
    } else {
        kad_behaviour.set_mode(Some(kad::Mode::Client));
    }

    // Create mDNS behavior wrapped in Toggle
    let mdns = match mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())
    {
        Ok(mdns_behaviour) => {
            if enable_mdns {
                Toggle::from(Some(mdns_behaviour))
            } else {
                Toggle::from(None)
            }
        }
        Err(e) => {
            tracing::warn!("Failed to create mDNS behavior: {}", e);
            Toggle::from(None)
        }
    };

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
    let xstream = XStreamNetworkBehaviour::new();

    // Create the network behavior
    NodeBehaviour {
        identify,
        kad: kad_behaviour,
        mdns,
        ping,
        por_auth,
        xstream,
    }
}

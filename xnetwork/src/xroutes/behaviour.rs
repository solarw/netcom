// src/xroutes/behaviour.rs

use std::num::NonZeroUsize;

use libp2p::{
    Multiaddr, PeerId, identity,
    kad::{self, BucketInserts, QueryId},
    mdns,
    swarm::{NetworkBehaviour, behaviour::toggle::Toggle},
};

use super::{
    XRoutesConfig,
    discovery::{behaviour::DiscoveryBehaviour, events::DiscoveryEvent},
    connectivity::{behaviour::{ConnectivityBehaviour, ConnectivityBehaviourEvent}, events::ConnectivityEvent},
};

// XRoutes behaviour using integrated discovery and connectivity systems
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "XRoutesDiscoveryBehaviourEvent")]
pub struct XRoutesDiscoveryBehaviour {
    pub discovery: DiscoveryBehaviour,
    pub connectivity: ConnectivityBehaviour,
}

// Events from XRoutes behaviour - discovery and connectivity events
#[derive(Debug)]
pub enum XRoutesDiscoveryBehaviourEvent {
    Discovery(DiscoveryEvent),
    Connectivity(ConnectivityBehaviourEvent),
}

impl From<DiscoveryEvent> for XRoutesDiscoveryBehaviourEvent {
    fn from(event: DiscoveryEvent) -> Self {
        XRoutesDiscoveryBehaviourEvent::Discovery(event)
    }
}

impl From<ConnectivityBehaviourEvent> for XRoutesDiscoveryBehaviourEvent {
    fn from(event: ConnectivityBehaviourEvent) -> Self {
        XRoutesDiscoveryBehaviourEvent::Connectivity(event)
    }
}

impl XRoutesDiscoveryBehaviour {
    pub fn new(
        key: &identity::Keypair,
        config: XRoutesConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let local_peer_id = PeerId::from(key.public());
        
        Ok(XRoutesDiscoveryBehaviour {
            discovery: DiscoveryBehaviour::new(
                key,
                config.enable_mdns,
                config.enable_kad,
                config.kad_server_mode,
            )?,
            connectivity: ConnectivityBehaviour::new_with_config(local_peer_id, &config),
        })
    }
}

// Re-export KadStats from discovery for compatibility
pub use super::discovery::kad::commands::KadStats;

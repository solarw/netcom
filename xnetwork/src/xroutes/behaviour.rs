// src/xroutes/behaviour.rs

use std::num::NonZeroUsize;

use libp2p::{
    identity,
    kad::{self, BucketInserts},
    mdns,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
};

use super::XRoutesConfig;

// XRoutes discovery behaviour combining mDNS and Kademlia
#[derive(NetworkBehaviour)]
pub struct XRoutesDiscoveryBehaviour {
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: Toggle<mdns::tokio::Behaviour>,
}

// Events from XRoutes behaviour
#[derive(Debug)]
pub enum XRoutesBehaviourEvent {
    Mdns(mdns::Event),
    Kad(kad::Event),
}

impl XRoutesDiscoveryBehaviour {
    pub fn new(key: &identity::Keypair, config: XRoutesConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Configure Kademlia
        let mut kad_config = kad::Config::default();

        if config.kad_server_mode {
            // Server mode settings - more aggressive for bootstrap nodes
            kad_config.set_parallelism(NonZeroUsize::new(5).unwrap());
            kad_config.set_query_timeout(std::time::Duration::from_secs(20));
            kad_config.disjoint_query_paths(true);
            kad_config.set_kbucket_inserts(kad::BucketInserts::OnConnected);
            kad_config.set_automatic_bootstrap_throttle(None);
            kad_config.set_replication_factor(NonZeroUsize::new(5).unwrap());
        } else {
            // Client mode settings
            kad_config.set_parallelism(NonZeroUsize::new(3).unwrap());
            kad_config.set_query_timeout(std::time::Duration::from_secs(30));
        }

        kad_config.set_kbucket_inserts(BucketInserts::Manual);
        kad_config.set_periodic_bootstrap_interval(None);
        kad_config.set_automatic_bootstrap_throttle(None);

        // Create Kademlia behaviour
        let kad_store = kad::store::MemoryStore::new(key.public().to_peer_id());
        let mut kad_behaviour = kad::Behaviour::with_config(
            key.public().to_peer_id(),
            kad_store,
            kad_config,
        );

        if config.kad_server_mode {
            kad_behaviour.set_mode(Some(kad::Mode::Server));
        } else {
            kad_behaviour.set_mode(Some(kad::Mode::Client));
        }

        // Create mDNS behaviour wrapped in Toggle
        let mdns = if config.enable_mdns {
            match mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id()) {
                Ok(mdns_behaviour) => Toggle::from(Some(mdns_behaviour)),
                Err(e) => {
                    tracing::warn!("Failed to create mDNS behavior: {}", e);
                    Toggle::from(None)
                }
            }
        } else {
            Toggle::from(None)
        };

        Ok(XRoutesDiscoveryBehaviour {
            kad: kad_behaviour,
            mdns,
        })
    }

    /// Enable mDNS discovery
    pub fn enable_mdns(&mut self, key: &identity::Keypair) -> Result<(), Box<dyn std::error::Error>> {
        if self.mdns.is_enabled() {
            return Ok(());
        }

        match mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id()) {
            Ok(mdns_behaviour) => {
                self.mdns = Toggle::from(Some(mdns_behaviour));
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to enable mDNS behavior: {}", e);
                Err(Box::new(e))
            }
        }
    }

    /// Disable mDNS discovery
    pub fn disable_mdns(&mut self) {
        self.mdns = Toggle::from(None);
    }

    /// Check if mDNS is enabled
    pub fn is_mdns_enabled(&self) -> bool {
        self.mdns.is_enabled()
    }

    /// Bootstrap Kademlia DHT
    pub fn bootstrap(&mut self) -> Result<libp2p::kad::QueryId, libp2p::kad::NoKnownPeers> {
        self.kad.bootstrap()
    }

    /// Get known peers from Kademlia routing table
    pub fn get_known_peers(&mut self) -> Vec<(libp2p::PeerId, Vec<libp2p::Multiaddr>)> {
        let mut peers = Vec::new();
        
        for kbucket in self.kad.kbuckets() {
            for entry in kbucket.iter() {
                let addresses = entry.node.value.clone().into_vec();
                let peer_id = entry.node.key.into_preimage();
                if !addresses.is_empty() {
                    peers.push((peer_id, addresses));
                }
            }
        }
        
        peers
    }

    /// Get addresses for a specific peer from Kademlia
    pub fn get_peer_addresses(&mut self, peer_id: &libp2p::PeerId) -> Vec<libp2p::Multiaddr> {
        let mut addresses = Vec::new();

        for bucket in self.kad.kbuckets() {
            for entry in bucket.iter() {
                if entry.node.key.preimage() == peer_id {
                    addresses = entry.node.value.clone().into_vec();
                    break;
                }
            }
            if !addresses.is_empty() {
                break;
            }
        }

        addresses
    }

    /// Initiate a search for peer addresses
    pub fn find_peer(&mut self, peer_id: libp2p::PeerId) {
        self.kad.get_closest_peers(peer_id);
    }

    /// Add address to Kademlia routing table
    pub fn add_address(&mut self, peer_id: &libp2p::PeerId, addr: libp2p::Multiaddr) {
        self.kad.add_address(peer_id, addr);
    }
}
// src/xroutes/behaviour.rs

use std::num::NonZeroUsize;

use libp2p::{
    identity,
    kad::{self, BucketInserts, QueryId},
    mdns,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
    Multiaddr, PeerId,
};

use super::XRoutesConfig;

// XRoutes discovery behaviour combining mDNS and Kademlia
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "XRoutesDiscoveryBehaviourEvent")]
pub struct XRoutesDiscoveryBehaviour {
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: Toggle<mdns::tokio::Behaviour>,
}

// Events from XRoutes behaviour - manually defined to match NetworkBehaviour expectation
#[derive(Debug)]
pub enum XRoutesDiscoveryBehaviourEvent {
    Mdns(mdns::Event),
    Kad(kad::Event),
}

impl From<mdns::Event> for XRoutesDiscoveryBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        XRoutesDiscoveryBehaviourEvent::Mdns(event)
    }
}

impl From<kad::Event> for XRoutesDiscoveryBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        XRoutesDiscoveryBehaviourEvent::Kad(event)
    }
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
    pub fn bootstrap(&mut self) -> Result<QueryId, libp2p::kad::NoKnownPeers> {
        self.kad.bootstrap()
    }

    /// Get known peers from Kademlia routing table
    pub fn get_known_peers(&mut self) -> Vec<(PeerId, Vec<Multiaddr>)> {
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
    pub fn get_peer_addresses(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
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

    /// Initiate a search for peer addresses (returns QueryId for tracking)
    pub fn find_peer(&mut self, peer_id: PeerId) -> QueryId {
        self.kad.get_closest_peers(peer_id)
    }

    /// Add address to Kademlia routing table
    pub fn add_address(&mut self, peer_id: &PeerId, addr: Multiaddr) {
        self.kad.add_address(peer_id, addr);
    }

    /// Remove address from Kademlia routing table
    pub fn remove_address(&mut self, peer_id: &PeerId, addr: &Multiaddr) {
        self.kad.remove_address(peer_id, addr);
    }

    /// Get the current Kademlia mode
    pub fn kad_mode(&mut self) -> Option<kad::Mode> {
        // This is a simplified check - in reality you might want to store the mode
        // or check the routing table configuration
        if self.kad.kbuckets().count() > 0 {
            Some(kad::Mode::Client) // Default assumption
        } else {
            None
        }
    }

    /// Set Kademlia mode
    pub fn set_kad_mode(&mut self, mode: kad::Mode) {
        self.kad.set_mode(Some(mode));
    }

    /// Get statistics about the Kademlia routing table
    pub fn kad_stats(&mut self) -> KadStats {
        let mut total_peers = 0;
        let mut buckets_with_peers = 0;

        for bucket in self.kad.kbuckets() {
            let bucket_size = bucket.iter().count();
            if bucket_size > 0 {
                buckets_with_peers += 1;
                total_peers += bucket_size;
            }
        }

        KadStats {
            total_peers,
            active_buckets: buckets_with_peers,
            total_buckets: self.kad.kbuckets().count(),
        }
    }

    /// Check if a specific peer is in the routing table
    pub fn has_peer(&mut self, peer_id: &PeerId) -> bool {
        for bucket in self.kad.kbuckets() {
            for entry in bucket.iter() {
                if entry.node.key.preimage() == peer_id {
                    return true;
                }
            }
        }
        false
    }

    /// Get the closest peers to a given peer ID from local routing table
    pub fn get_closest_local_peers(&mut self, peer_id: &PeerId, count: usize) -> Vec<(PeerId, Vec<Multiaddr>)> {
        let mut peers = Vec::new();
        
        // This is a simplified implementation - Kademlia has more sophisticated
        // methods for finding closest peers based on XOR distance
        for bucket in self.kad.kbuckets() {
            for entry in bucket.iter() {
                let addresses = entry.node.value.clone().into_vec();
                let entry_peer_id = entry.node.key.into_preimage();
                if !addresses.is_empty() && entry_peer_id != *peer_id {
                    peers.push((entry_peer_id, addresses));
                }
                
                if peers.len() >= count {
                    break;
                }
            }
            if peers.len() >= count {
                break;
            }
        }
        
        peers
    }

    /// Force refresh of a specific bucket in the routing table
    pub fn refresh_bucket(&mut self, peer_id: PeerId) -> QueryId {
        // Initiate a query to refresh the bucket containing this peer
        self.kad.get_closest_peers(peer_id)
    }
}

/// Statistics about the Kademlia routing table
#[derive(Debug, Clone)]
pub struct KadStats {
    pub total_peers: usize,
    pub active_buckets: usize,
    pub total_buckets: usize,
}

impl KadStats {
    /// Calculate the average peers per active bucket
    pub fn avg_peers_per_bucket(&self) -> f32 {
        if self.active_buckets > 0 {
            self.total_peers as f32 / self.active_buckets as f32
        } else {
            0.0
        }
    }

    /// Calculate the fill ratio of the routing table
    pub fn fill_ratio(&self) -> f32 {
        if self.total_buckets > 0 {
            self.active_buckets as f32 / self.total_buckets as f32
        } else {
            0.0
        }
    }
}
//! XRoutes composite behaviour with toggle components
use std::time::Duration;

use libp2p::{
    identify,
    mdns,
    kad,
    swarm::behaviour::toggle::Toggle,
    swarm::NetworkBehaviour,
    PeerId,
};

use super::types::XROUTES_IDENTIFY_PROTOCOL;

/// Composite behaviour for XRoutes with toggle components
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "XRoutesBehaviourEvent")]
pub struct XRoutesBehaviour {
    /// Identify behaviour with custom protocol
    pub identify: Toggle<identify::Behaviour>,
    /// mDNS discovery behaviour
    pub mdns: Toggle<mdns::tokio::Behaviour>,
    /// Kademlia DHT discovery behaviour
    pub kad: Toggle<kad::Behaviour<kad::store::MemoryStore>>,
}

/// Events emitted by XRoutesBehaviour
#[derive(Debug)]
pub enum XRoutesBehaviourEvent {
    /// Identify behaviour event
    Identify(identify::Event),
    /// mDNS behaviour event
    Mdns(mdns::Event),
    /// Kademlia behaviour event
    Kad(kad::Event),
}

impl From<identify::Event> for XRoutesBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        XRoutesBehaviourEvent::Identify(event)
    }
}

impl From<mdns::Event> for XRoutesBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        XRoutesBehaviourEvent::Mdns(event)
    }
}

impl From<kad::Event> for XRoutesBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        XRoutesBehaviourEvent::Kad(event)
    }
}

impl XRoutesBehaviour {
    /// Create a new XRoutesBehaviour with optional components
    pub fn new(
        local_peer_id: PeerId,
        config: &super::types::XRoutesConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create identify behaviour - disabled since it's already in main behaviour
        let identify = Toggle::from(None);

        // Create mDNS behaviour
        let mdns = if config.enable_mdns {
            Toggle::from(Some(mdns::tokio::Behaviour::new(
                mdns::Config::default(),
                local_peer_id,
            )?))
        } else {
            Toggle::from(None)
        };

        // Create Kademlia behaviour
        let kad = if config.enable_kad {
            let store = kad::store::MemoryStore::new(local_peer_id);
            let mut kad_config = kad::Config::default();
            kad_config.set_query_timeout(Duration::from_secs(3*60));
            kad_config.set_periodic_bootstrap_interval(Some(Duration::from_secs(5)));
            kad_config.set_replication_interval(Some(Duration::from_secs(5)));
            kad_config.set_provider_publication_interval(Some(Duration::from_secs(5)));
            Toggle::from(Some(kad::Behaviour::with_config(
                local_peer_id,
                store,
                kad_config,
            )))
        } else {
            Toggle::from(None)
        };

        Ok(Self {
            identify,
            mdns,
            kad,
        })
    }

    /// Enable identify behaviour
    pub fn enable_identify(&mut self, config: identify::Config) {
        self.identify = Toggle::from(Some(identify::Behaviour::new(config)));
    }

    /// Disable identify behaviour
    pub fn disable_identify(&mut self) {
        self.identify = Toggle::from(None);
    }

    /// Enable mDNS behaviour
    pub fn enable_mdns(&mut self, local_peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.mdns = Toggle::from(Some(mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        )?));
        Ok(())
    }

    /// Disable mDNS behaviour
    pub fn disable_mdns(&mut self) {
        self.mdns = Toggle::from(None);
    }

    /// Enable Kademlia behaviour
    pub fn enable_kad(&mut self, local_peer_id: PeerId) {
        let store = kad::store::MemoryStore::new(local_peer_id);
        let kad_config = kad::Config::default();
        self.kad = Toggle::from(Some(kad::Behaviour::with_config(
            local_peer_id,
            store,
            kad_config,
        )));
    }

    /// Disable Kademlia behaviour
    pub fn disable_kad(&mut self) {
        self.kad = Toggle::from(None);
    }

    /// Get current status of behaviours
    pub fn get_status(&self) -> super::types::XRoutesStatus {
        super::types::XRoutesStatus {
            identify_enabled: self.identify.as_ref().is_some(),
            mdns_enabled: self.mdns.as_ref().is_some(),
            kad_enabled: self.kad.as_ref().is_some(),
        }
    }
}

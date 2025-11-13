//! XRoutes composite behaviour with toggle components
use std::time::Duration;

use libp2p::{
    PeerId,
    identify::{self, Config},
    identity::PublicKey,
    kad, mdns, relay,
    swarm::{NetworkBehaviour, behaviour::toggle::Toggle},
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
    /// Relay server behaviour
    pub relay_server: Toggle<relay::Behaviour>,
    /// Relay client behaviour
    pub relay_client: Toggle<relay::client::Behaviour>,
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
    /// Relay server behaviour event
    RelayServer(relay::Event),
    /// Relay client behaviour event
    RelayClient(relay::client::Event),
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

impl From<relay::Event> for XRoutesBehaviourEvent {
    fn from(event: relay::Event) -> Self {
        XRoutesBehaviourEvent::RelayServer(event)
    }
}

impl From<relay::client::Event> for XRoutesBehaviourEvent {
    fn from(event: relay::client::Event) -> Self {
        XRoutesBehaviourEvent::RelayClient(event)
    }
}

impl XRoutesBehaviour {
    /// Create a new XRoutesBehaviour with optional components
    pub fn new(
        local_public_key: PublicKey,
        config: &super::types::XRoutesConfig,
        relay_client: Option<relay::client::Behaviour>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create identify behaviour - disabled since it's already in main behaviour

        let local_peer_id = local_public_key.to_peer_id();
        // Create mDNS behaviour

        let identify = if config.enable_identify {
           Toggle::from(Some(XRoutesBehaviour::make_identify_behaviour(local_public_key)))
        } else {
            Toggle::from(None)
        };

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
            kad_config.set_query_timeout(Duration::from_secs(3 * 60));
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

        // Create relay server behaviour
        let relay_server = if config.enable_relay_server {
            Toggle::from(Some(relay::Behaviour::new(
                local_peer_id,
                Default::default(),
            )))
        } else {
            Toggle::from(None)
        };

        // Create relay client behaviour from Option
        let relay_client = if let Some(relay_client_behaviour) = relay_client {
            Toggle::from(Some(relay_client_behaviour))
        } else {
            Toggle::from(None)
        };

        Ok(Self {
            identify,
            mdns,
            kad,
            relay_server,
            relay_client,
        })
    }

    pub fn make_identify_behaviour(local_public_key: PublicKey) -> identify::Behaviour {
        let mut config =
            identify::Config::new(XROUTES_IDENTIFY_PROTOCOL.to_string(), local_public_key)
                .with_push_listen_addr_updates(true);
        identify::Behaviour::new(config)
    }
    /// Enable identify behaviour
    pub fn enable_identify(&mut self, local_public_key: PublicKey) {
        self.identify = Toggle::from(Some(XRoutesBehaviour::make_identify_behaviour(local_public_key)));
    }

    /// Disable identify behaviour
    pub fn disable_identify(&mut self) {
        self.identify = Toggle::from(None);
    }

    /// Enable mDNS behaviour
    pub fn enable_mdns(
        &mut self,
        local_peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            relay_server_enabled: self.relay_server.as_ref().is_some(),
            relay_client_enabled: self.relay_client.as_ref().is_some(),
        }
    }

    /// Enable relay server behaviour
    pub fn enable_relay_server(&mut self, local_peer_id: PeerId) {
        self.relay_server = Toggle::from(Some(relay::Behaviour::new(
            local_peer_id,
            Default::default(),
        )));
    }

    /// Disable relay server behaviour
    pub fn disable_relay_server(&mut self) {
        self.relay_server = Toggle::from(None);
    }
}

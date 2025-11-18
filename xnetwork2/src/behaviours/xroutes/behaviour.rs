//! XRoutes composite behaviour with toggle components
use std::time::Duration;

use libp2p::{
    PeerId,
    identify::{self, Config},
    identity::PublicKey,
    kad, mdns, relay,
    swarm::{NetworkBehaviour, behaviour::toggle::Toggle},
};
use libp2p::autonat::v2;

use super::types::{XROUTES_IDENTIFY_PROTOCOL, KadMode};
use crate::connection_tracker::ConnectionTracker;

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
    /// DCUtR behaviour for hole punching
    pub dcutr: Toggle<libp2p::dcutr::Behaviour>,
    /// AutoNAT v2 client behaviour for NAT type detection
    pub autonat_client: Toggle<v2::client::Behaviour>,
    /// AutoNAT v2 server behaviour for providing NAT detection services
    pub autonat_server: Toggle<v2::server::Behaviour>,
    /// Connection tracker behaviour for monitoring connections
    pub connection_tracker: Toggle<ConnectionTracker>,
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
    /// DCUtR behaviour event
    Dcutr(libp2p::dcutr::Event),
    /// AutoNAT v2 client behaviour event
    AutonatClient(v2::client::Event),
    /// AutoNAT v2 server behaviour event
    AutonatServer(v2::server::Event),
    /// Empty event for NetworkBehaviour requirements
    Empty,
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

impl From<libp2p::dcutr::Event> for XRoutesBehaviourEvent {
    fn from(event: libp2p::dcutr::Event) -> Self {
        XRoutesBehaviourEvent::Dcutr(event)
    }
}

impl From<v2::client::Event> for XRoutesBehaviourEvent {
    fn from(event: v2::client::Event) -> Self {
        XRoutesBehaviourEvent::AutonatClient(event)
    }
}

impl From<v2::server::Event> for XRoutesBehaviourEvent {
    fn from(event: v2::server::Event) -> Self {
        XRoutesBehaviourEvent::AutonatServer(event)
    }
}

impl From<()> for XRoutesBehaviourEvent {
    fn from(_: ()) -> Self {
        XRoutesBehaviourEvent::Empty
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

        // Create DCUtR behaviour
        let dcutr = if config.enable_dcutr {
            Toggle::from(Some(libp2p::dcutr::Behaviour::new(local_peer_id)))
        } else {
            Toggle::from(None)
        };

        // Create AutoNAT v2 client behaviour
        let autonat_client = if config.enable_autonat_client {
            Toggle::from(Some(v2::client::Behaviour::default()))
        } else {
            Toggle::from(None)
        };

        // Create AutoNAT v2 server behaviour
        let autonat_server = if config.enable_autonat_server {
            Toggle::from(Some(v2::server::Behaviour::default()))
        } else {
            Toggle::from(None)
        };

        // Create ConnectionTracker behaviour
        let connection_tracker = if config.enable_connection_tracking {
            Toggle::from(Some(ConnectionTracker::new(local_peer_id)))
        } else {
            Toggle::from(None)
        };

        Ok(Self {
            identify,
            mdns,
            kad,
            relay_server,
            relay_client,
            dcutr,
            autonat_client,
            autonat_server,
            connection_tracker,
        })
    }

    pub fn make_identify_behaviour(local_public_key: PublicKey) -> identify::Behaviour {
        let mut config =
            identify::Config::new(XROUTES_IDENTIFY_PROTOCOL.to_string(), local_public_key)
                .with_push_listen_addr_updates(true);
        identify::Behaviour::new(config)
    }

    /// Enable AutoNAT v2 client behaviour
    pub fn enable_autonat_client(&mut self) {
        self.autonat_client = Toggle::from(Some(v2::client::Behaviour::default()));
    }

    /// Disable AutoNAT v2 client behaviour
    pub fn disable_autonat_client(&mut self) {
        self.autonat_client = Toggle::from(None);
    }

    /// Enable AutoNAT v2 server behaviour
    pub fn enable_autonat_server(&mut self) {
        self.autonat_server = Toggle::from(Some(v2::server::Behaviour::default()));
    }

    /// Disable AutoNAT v2 server behaviour
    pub fn disable_autonat_server(&mut self) {
        self.autonat_server = Toggle::from(None);
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
        let kad_mode = if let Some(kad_behaviour) = self.kad.as_ref() {
            match kad_behaviour.mode() {
                kad::Mode::Client => Some(super::types::KadMode::Client),
                kad::Mode::Server => Some(super::types::KadMode::Server),
            }
        } else {
            None
        };

        super::types::XRoutesStatus {
            identify_enabled: self.identify.as_ref().is_some(),
            mdns_enabled: self.mdns.as_ref().is_some(),
            kad_enabled: self.kad.as_ref().is_some(),
            kad_mode,
            relay_server_enabled: self.relay_server.as_ref().is_some(),
            relay_client_enabled: self.relay_client.as_ref().is_some(),
            dcutr_enabled: self.dcutr.as_ref().is_some(),
            autonat_server_enabled: self.autonat_server.as_ref().is_some(),
            autonat_client_enabled: self.autonat_client.as_ref().is_some(),
            connection_tracking_enabled: self.connection_tracker.as_ref().is_some(),
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

    /// Set Kademlia mode (client, server, auto)
    pub fn set_kad_mode(&mut self, mode: KadMode) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(kad_behaviour) = self.kad.as_mut() {
            match mode {
                KadMode::Client => {
                    kad_behaviour.set_mode(Some(kad::Mode::Client));
                }
                KadMode::Server => {
                    kad_behaviour.set_mode(Some(kad::Mode::Server));
                }
                KadMode::Auto => {
                    kad_behaviour.set_mode(None); // None enables auto mode
                }
            }
            Ok(())
        } else {
            Err("Kademlia behaviour is not enabled".into())
        }
    }

    /// Get current Kademlia mode
    pub fn get_kad_mode(&self) -> Result<KadMode, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(kad_behaviour) = self.kad.as_ref() {
            let mode = kad_behaviour.mode();
            match mode {
                kad::Mode::Client => Ok(KadMode::Client),
                kad::Mode::Server => Ok(KadMode::Server),
            }
        } else {
            Err("Kademlia behaviour is not enabled".into())
        }
    }
}

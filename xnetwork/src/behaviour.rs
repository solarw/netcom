// src/behaviour.rs

use libp2p::{
    identify, identity, ping,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
};

use xauth::behaviours::PorAuthBehaviour;
use xauth::por::por::ProofOfRepresentation;
use xstream::behaviour::XStreamNetworkBehaviour;

use crate::xroutes::{XRoutesDiscoveryBehaviour, XRoutesDiscoveryBehaviourEvent, XRoutesConfig};

// Main node behaviour combining all protocols
#[derive(NetworkBehaviour)]
pub struct NodeBehaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub por_auth: PorAuthBehaviour,
    pub xstream: XStreamNetworkBehaviour,
    // XRoutes discovery is optional - use Toggle to make it optional
    pub xroutes: libp2p::swarm::behaviour::toggle::Toggle<XRoutesDiscoveryBehaviour>,
}

/// Create the main node behaviour
pub fn make_behaviour(
    key: &identity::Keypair,
    por: ProofOfRepresentation,
    enable_mdns: bool,
    kad_server_mode: bool,
) -> Result<NodeBehaviour, Box<dyn std::error::Error + Send + Sync>> {
    // Set up the Identify protocol
    let identify = identify::Behaviour::new(
        identify::Config::new("/ipfs/id/1.0.0".to_string(), key.public())
            .with_agent_version(format!("p2p-network/{}", env!("CARGO_PKG_VERSION"))),
    );

    // Set up ping behavior for connection keep-alive
    let ping = ping::Behaviour::new(ping::Config::default());

    // Set up PoR authentication behavior
    let por_auth = PorAuthBehaviour::new(por);

    // Set up stream behavior
    let xstream = XStreamNetworkBehaviour::new();

    // Set up XRoutes discovery (optional)
    let xroutes = if enable_mdns || kad_server_mode {
        let config = XRoutesConfig {
            enable_mdns,
            enable_kad: true,
            kad_server_mode,
            initial_role: if kad_server_mode {
                crate::xroutes::XRouteRole::Server
            } else {
                crate::xroutes::XRouteRole::Client
            },
        };
        
        match XRoutesDiscoveryBehaviour::new(key, config) {
            Ok(behaviour) => libp2p::swarm::behaviour::toggle::Toggle::from(Some(behaviour)),
            Err(e) => {
                tracing::warn!("Failed to create XRoutes discovery behaviour: {}", e);
                libp2p::swarm::behaviour::toggle::Toggle::from(None)
            }
        }
    } else {
        libp2p::swarm::behaviour::toggle::Toggle::from(None)
    };

    Ok(NodeBehaviour {
        identify,
        ping,
        por_auth,
        xstream,
        xroutes,
    })
}

/// Create the main node behaviour with extended configuration
pub fn make_behaviour_with_config(
    key: &identity::Keypair,
    por: ProofOfRepresentation,
    xroutes_config: Option<XRoutesConfig>,
) -> Result<NodeBehaviour, Box<dyn std::error::Error + Send + Sync>> {
    // Set up the Identify protocol
    let identify = identify::Behaviour::new(
        identify::Config::new("/ipfs/id/1.0.0".to_string(), key.public())
            .with_agent_version(format!("p2p-network/{}", env!("CARGO_PKG_VERSION"))),
    );

    // Set up ping behavior for connection keep-alive
    let ping = ping::Behaviour::new(ping::Config::default());

    // Set up PoR authentication behavior
    let por_auth = PorAuthBehaviour::new(por);

    // Set up stream behavior
    let xstream = XStreamNetworkBehaviour::new();

    // Set up XRoutes discovery if config provided
    let xroutes = if let Some(config) = xroutes_config {
        match XRoutesDiscoveryBehaviour::new(key, config) {
            Ok(behaviour) => libp2p::swarm::behaviour::toggle::Toggle::from(Some(behaviour)),
            Err(e) => {
                tracing::warn!("Failed to create XRoutes discovery behaviour: {}", e);
                libp2p::swarm::behaviour::toggle::Toggle::from(None)
            }
        }
    } else {
        libp2p::swarm::behaviour::toggle::Toggle::from(None)
    };

    Ok(NodeBehaviour {
        identify,
        ping,
        por_auth,
        xstream,
        xroutes,
    })
}
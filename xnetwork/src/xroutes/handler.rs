// src/xroutes/handler.rs

use libp2p::{Multiaddr, PeerId, Swarm, identity, kad};
use std::collections::HashMap;
use tracing::info;

use crate::behaviour::NodeBehaviour;

use super::connectivity::behaviour::ConnectivityBehaviourEvent;
use super::discovery::events::DiscoveryEvent;
use super::{
    XRoutesConfig,
    behaviour::XRoutesDiscoveryBehaviourEvent,
    commands::XRoutesCommand,
    events::XRoutesEvent,
    types::{BootstrapError, BootstrapNodeInfo},
    xroute::XRouteRole,
};

pub struct XRoutesHandler {
    current_role: XRouteRole,
    bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
    discovered_peers: HashMap<PeerId, Vec<Multiaddr>>,
    config: XRoutesConfig,
}

impl XRoutesHandler {
    pub fn new(config: XRoutesConfig) -> Self {
        Self {
            current_role: config.initial_role.clone(),
            bootstrap_nodes: Vec::new(),
            discovered_peers: HashMap::new(),
            config,
        }
    }

    /// Handle XRoutes commands
    pub async fn handle_command(
        &mut self,
        cmd: XRoutesCommand,
        swarm: &mut Swarm<NodeBehaviour>,
        _key: &identity::Keypair,
    ) {
        match cmd {
            XRoutesCommand::Discovery(discover_cmd) => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.discovery.handle_command(discover_cmd);
                }
            }

            XRoutesCommand::Connectivity(connectivity_cmd) => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.connectivity.handle_command(connectivity_cmd);
                }
            }

            XRoutesCommand::SetRole { role, response } => {
                let result = self.set_role(role, swarm).await;
                let _ = response.send(result);
            }

            XRoutesCommand::GetRole { response } => {
                let _ = response.send(self.current_role.clone());
            }

            XRoutesCommand::ConnectToBootstrap {
                addr,
                timeout_secs,
                response,
            } => {
                let result = self
                    .handle_bootstrap_connection(addr, timeout_secs, swarm)
                    .await;
                let _ = response.send(result);
            }
        }
    }

    /// Handle XRoutes behaviour events and return network events
    pub async fn handle_behaviour_event(
        &mut self,
        event: XRoutesDiscoveryBehaviourEvent,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        match event {
            XRoutesDiscoveryBehaviourEvent::Discovery(discovery_event) => {
                // Handle discovery events
                self.handle_discovery_event(discovery_event, swarm).await
            }
            XRoutesDiscoveryBehaviourEvent::Connectivity(connectivity_event) => {
                // Handle connectivity events
                self.handle_connectivity_event(connectivity_event, swarm)
                    .await
            }
        }
    }

    /// Handle discovery events
    async fn handle_discovery_event(
        &mut self,
        event: DiscoveryEvent,
        _swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        // Convert discovery events to XRoutes events
        vec![XRoutesEvent::DiscoveryEvent(event)]
    }

    /// Handle connectivity events
    async fn handle_connectivity_event(
        &mut self,
        event: ConnectivityBehaviourEvent,
        _swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        // Convert connectivity events to XRoutes events
        match event {
            // Relay server events are no longer supported - relay functionality has been removed
            ConnectivityBehaviourEvent::ConnectivityEvent(connectivity_event) => {
                vec![XRoutesEvent::ConnectivityEvent(connectivity_event)]
            }
        }
    }

    /// Set XRoute role
    async fn set_role(
        &mut self,
        new_role: XRouteRole,
        _swarm: &mut Swarm<NodeBehaviour>,
    ) -> Result<(), String> {
        let old_role = self.current_role.clone();

        if old_role == new_role {
            return Ok(());
        }

        self.current_role = new_role.clone();

        info!("XRoute role changed from {:?} to {:?}", old_role, new_role);
        Ok(())
    }

    /// Get current role
    pub fn current_role(&self) -> &XRouteRole {
        &self.current_role
    }

    /// Handle bootstrap connection
    async fn handle_bootstrap_connection(
        &mut self,
        addr: Multiaddr,
        timeout_secs: Option<u64>,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        use tokio::time::{Duration, timeout};

        let timeout_duration = Duration::from_secs(timeout_secs.unwrap_or(30));

        timeout(timeout_duration, async {
            // Extract peer_id from multiaddr
            let peer_id = extract_peer_id_from_addr(&addr).ok_or_else(|| {
                BootstrapError::InvalidAddress("Address must contain PeerId".to_string())
            })?;

            // Establish connection
            swarm
                .dial(addr.clone())
                .map_err(|e| BootstrapError::ConnectionFailed(format!("Dial failed: {}", e)))?;

            // Wait for connection to establish
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // For now, assume it's a server if connection succeeded
            let role = XRouteRole::Server;

            // Verify it's actually a server
            if role != XRouteRole::Server {
                return Err(BootstrapError::NotABootstrapServer(role));
            }

            // Add to bootstrap nodes list
            self.bootstrap_nodes.push((peer_id, addr));

            Ok(BootstrapNodeInfo {
                peer_id,
                role,
                protocols: vec!["kad".to_string()],
                agent_version: "unknown".to_string(),
            })
        })
        .await
        .map_err(|_| BootstrapError::ConnectionTimeout)?
    }
}

// Helper function to extract peer ID from multiaddr
fn extract_peer_id_from_addr(addr: &Multiaddr) -> Option<PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}

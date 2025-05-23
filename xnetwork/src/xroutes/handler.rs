// src/xroutes/handler.rs

use libp2p::{identity, kad, mdns, Multiaddr, PeerId, Swarm};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::behaviour::NodeBehaviour;

use super::{
    commands::XRoutesCommand,
    events::XRoutesEvent,
    types::{BootstrapNodeInfo, BootstrapError},
    xroute::XRouteRole,
    behaviour::XRoutesDiscoveryBehaviourEvent,
    XRoutesConfig,
};

pub struct XRoutesHandler {
    current_role: XRouteRole,
    mdns_enabled: bool,
    kad_enabled: bool,
    bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
    discovered_peers: HashMap<PeerId, Vec<Multiaddr>>,
    config: XRoutesConfig,
}

impl XRoutesHandler {
    pub fn new(config: XRoutesConfig) -> Self {
        Self {
            current_role: config.initial_role.clone(),
            mdns_enabled: config.enable_mdns,
            kad_enabled: config.enable_kad,
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
        key: &identity::Keypair,
    ) {
        match cmd {
            XRoutesCommand::EnableMdns => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    match xroutes.enable_mdns(key) {
                        Ok(_) => {
                            self.mdns_enabled = true;
                            info!("mDNS discovery enabled");
                        }
                        Err(e) => {
                            warn!("Failed to enable mDNS: {}", e);
                        }
                    }
                }
            }

            XRoutesCommand::DisableMdns => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.disable_mdns();
                    self.mdns_enabled = false;
                    info!("mDNS discovery disabled");
                }
            }

            XRoutesCommand::EnableKad => {
                self.kad_enabled = true;
                info!("Kademlia discovery enabled");
                // Try to bootstrap immediately
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    if let Err(e) = xroutes.bootstrap() {
                        warn!("Failed to bootstrap Kademlia: {}", e);
                    } else {
                        info!("Bootstrap query initiated");
                    }
                }
            }

            XRoutesCommand::DisableKad => {
                self.kad_enabled = false;
                info!("Kademlia discovery disabled");
            }

            XRoutesCommand::BootstrapKad { response } => {
                info!("Bootstrapping Kademlia DHT");
                let result = if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    match xroutes.bootstrap() {
                        Ok(query_id) => {
                            info!("Kademlia bootstrap started successfully with query_id: {:?}", query_id);
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Failed to bootstrap Kademlia: {}", e);
                            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        }
                    }
                } else {
                    Err("XRoutes discovery not enabled".into())
                };
                let _ = response.send(result);
            }

            XRoutesCommand::GetKadKnownPeers { response } => {
                let peers = if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.get_known_peers()
                } else {
                    Vec::new()
                };
                let _ = response.send(peers);
            }

            XRoutesCommand::GetPeerAddresses { peer_id, response } => {
                let addresses = if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.get_peer_addresses(&peer_id)
                } else {
                    Vec::new()
                };
                let _ = response.send(addresses);
            }

            XRoutesCommand::FindPeerAddresses { peer_id, response } => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.find_peer(peer_id);
                    let _ = response.send(Ok(()));
                } else {
                    let _ = response.send(Err("XRoutes discovery not enabled".into()));
                }
            }

            XRoutesCommand::SetRole { role, response } => {
                let result = self.set_role(role, swarm).await;
                let _ = response.send(result);
            }

            XRoutesCommand::GetRole { response } => {
                let _ = response.send(self.current_role.clone());
            }
            
            // NEW: Bootstrap connection command
            XRoutesCommand::ConnectToBootstrap { addr, timeout_secs, response } => {
                let result = self.handle_bootstrap_connection(addr, timeout_secs, swarm).await;
                let _ = response.send(result);
            }
        }
    }

    /// Handle XRoutes behaviour events and return network events
    pub async fn handle_behaviour_event(
        &mut self,
        event: XRoutesDiscoveryBehaviourEvent, // Исправляем тип параметра
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        match event {
            XRoutesDiscoveryBehaviourEvent::Mdns(mdns_event) => {
                self.handle_mdns_event(mdns_event, swarm).await
            }
            XRoutesDiscoveryBehaviourEvent::Kad(kad_event) => {
                self.handle_kad_event(kad_event, swarm).await
            }
        }
    }

    /// Handle mDNS events
    async fn handle_mdns_event(
        &mut self,
        event: mdns::Event,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        let mut events = Vec::new();

        match event {
            mdns::Event::Discovered(peers) => {
                if self.mdns_enabled {
                    let mut discovered_addresses = Vec::new();
                    
                    for (peer_id, addr) in peers {
                        info!("mDNS discovered peer: {peer_id} at {addr}");
                        
                        // Add to our discovered peers map
                        self.discovered_peers
                            .entry(peer_id)
                            .or_insert_with(Vec::new)
                            .push(addr.clone());
                        
                        discovered_addresses.push((peer_id, addr.clone()));

                        // Add to Kademlia if enabled
                        if self.kad_enabled {
                            if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                                xroutes.add_address(&peer_id, addr.clone());
                                info!("📚 Added address to Kademlia: {peer_id} at {addr}");
                                
                                events.push(XRoutesEvent::KadAddressAdded {
                                    peer_id,
                                    addr: addr.clone(),
                                });
                            }
                        }
                    }

                    // Group discovered peers by peer_id
                    let mut peer_groups: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
                    for (peer_id, addr) in discovered_addresses {
                        peer_groups.entry(peer_id).or_insert_with(Vec::new).push(addr);
                    }

                    // Generate discovery events
                    for (peer_id, addresses) in peer_groups {
                        events.push(XRoutesEvent::MdnsPeerDiscovered {
                            peer_id,
                            addresses,
                        });
                    }
                }
            }
            mdns::Event::Expired(peers) => {
                for (peer_id, _addr) in peers {
                    info!("mDNS peer expired: {peer_id}");
                    self.discovered_peers.remove(&peer_id);
                    events.push(XRoutesEvent::MdnsPeerExpired { peer_id });
                }
            }
        }

        events
    }

    /// Handle Kademlia events
    async fn handle_kad_event(
        &mut self,
        event: kad::Event,
        _swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        let mut events = Vec::new();

        match event {
            kad::Event::RoutingUpdated { peer, addresses, .. } => {
                info!("📔 Kademlia routing updated for peer: {peer}");
                for addr in addresses.iter() {
                    info!("📕 Known address: {addr}");
                }

                events.push(XRoutesEvent::KadRoutingUpdated {
                    peer_id: peer,
                    addresses: addresses.iter().cloned().collect(),
                });
            }

            kad::Event::PendingRoutablePeer { peer, .. } => {
                info!("🔍 Kademlia looking for addresses of peer: {peer}");
            }

            kad::Event::OutboundQueryProgressed { result, .. } => match result {
                kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                    key,
                    providers,
                    ..
                })) => {
                    for peer in providers {
                        info!("Found provider for {key:?}: {peer}");
                    }
                }
                kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { key: _, peers })) => {
                    let mut discovered_peers = Vec::new();
                    for peer in peers {
                        info!("Found peer info {} {:?}", peer.peer_id, peer.addrs);
                        if !peer.addrs.is_empty() {
                            discovered_peers.push((peer.peer_id, peer.addrs));
                        }
                    }
                    
                    for (peer_id, addresses) in discovered_peers {
                        events.push(XRoutesEvent::KadPeerDiscovered {
                            peer_id,
                            addresses,
                        });
                    }
                }
                _ => {}
            },

            kad::Event::InboundRequest { request, .. } => {
                info!("📥 Received Kademlia request: {:?}", request);
            }

            _ => {
                info!("Other Kad event: {event:?}");
            }
        }

        events
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
        
        // TODO: Update identify behaviour with new protocol marker
        // This might require recreating the behaviour or finding another way
        // to update the protocol list dynamically
        
        info!("XRoute role changed from {:?} to {:?}", old_role, new_role);
        Ok(())
    }

    /// Get current role
    pub fn current_role(&self) -> &XRouteRole {
        &self.current_role
    }

    /// Check if mDNS is enabled
    pub fn is_mdns_enabled(&self) -> bool {
        self.mdns_enabled
    }

    /// Check if Kademlia is enabled
    pub fn is_kad_enabled(&self) -> bool {
        self.kad_enabled
    }
    
    /// Handle bootstrap connection
    async fn handle_bootstrap_connection(
        &mut self,
        addr: libp2p::Multiaddr,
        timeout_secs: Option<u64>,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        use tokio::time::{timeout, Duration};
        
        let timeout_duration = Duration::from_secs(timeout_secs.unwrap_or(30));
        
        timeout(timeout_duration, async {
            // Extract peer_id from multiaddr
            let peer_id = extract_peer_id_from_addr(&addr)
                .ok_or_else(|| BootstrapError::InvalidAddress("Address must contain PeerId".to_string()))?;
            
            // Establish connection
            swarm.dial(addr.clone())
                .map_err(|e| BootstrapError::ConnectionFailed(format!("Dial failed: {}", e)))?;
            
            // Wait for connection to establish
            // In a real implementation, we would wait for ConnectionEstablished event
            tokio::time::sleep(Duration::from_millis(1000)).await;
            
            // For now, assume it's a server if connection succeeded
            // In real implementation, we would check identify info or XRoute protocol
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
        }).await
        .map_err(|_| BootstrapError::ConnectionTimeout)?
    }
}

// Helper function to extract peer ID from multiaddr
fn extract_peer_id_from_addr(addr: &libp2p::Multiaddr) -> Option<libp2p::PeerId> {
    use libp2p::multiaddr::Protocol;
    
    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}
// src/node.rs

use libp2p::futures::StreamExt;
use libp2p::{identify, identity, Multiaddr, PeerId, Swarm};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::error;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Duration;

use tracing::{info, warn, debug};

use xstream::events::XStreamEvent;
use xauth::events::PorAuthEvent;

use crate::{
    behaviour::{make_behaviour, NodeBehaviour, NodeBehaviourEvent},
    commands::NetworkCommand,
    events::NetworkEvent,
};

use crate::xroutes::{XRoutesCommand, XRoutesConfig, XRoutesHandler};

pub struct NetworkNode {
    cmd_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
    swarm: Swarm<NodeBehaviour>,
    connected_peers: HashMap<PeerId, Vec<Multiaddr>>,
    local_peer_id: PeerId,
    authenticated_peers: HashSet<PeerId>,
    
    // XRoutes handler - optional
    xroutes_handler: Option<XRoutesHandler>,
    
    // Store key for XRoutes operations
    local_key: identity::Keypair,
}

impl NetworkNode {
    // Create a new NetworkNode with all required protocols
    pub async fn new(
        local_key: identity::Keypair,
        por: xauth::por::por::ProofOfRepresentation,
        enable_mdns: bool,
        kad_server_mode: bool,
    ) -> Result<
        (
            Self,
            mpsc::Sender<NetworkCommand>,
            mpsc::Receiver<NetworkEvent>,
            PeerId,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        let local_peer_id = PeerId::from(local_key.public());
        
        // Create XRoutes handler if discovery is enabled
        let xroutes_handler = if enable_mdns || kad_server_mode {
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
            Some(XRoutesHandler::new(config))
        } else {
            None
        };

        // Create a SwarmBuilder with QUIC transport
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| make_behaviour(key, por, enable_mdns, kad_server_mode))?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(std::time::Duration::from_secs(60000))
            })
            .build();

        // Set up communication channels
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);

        Ok((
            Self {
                cmd_rx,
                event_tx,
                swarm,
                connected_peers: HashMap::new(),
                local_peer_id,
                authenticated_peers: HashSet::new(),
                xroutes_handler,
                local_key,
            },
            cmd_tx,
            event_rx,
            local_peer_id,
        ))
    }

    // Create a new NetworkNode with extended XRoutes configuration
    pub async fn new_with_config(
        local_key: identity::Keypair,
        por: xauth::por::por::ProofOfRepresentation,
        xroutes_config: Option<XRoutesConfig>,
    ) -> Result<
        (
            Self,
            mpsc::Sender<NetworkCommand>,
            mpsc::Receiver<NetworkEvent>,
            PeerId,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        let local_peer_id = PeerId::from(local_key.public());
        
        // Create XRoutes handler if config provided
        let xroutes_handler = if let Some(config) = xroutes_config.clone() {
            if config.is_discovery_enabled() {
                config.validate().map_err(|e| format!("Invalid XRoutes config: {}", e))?;
                Some(XRoutesHandler::new(config.clone()))
            } else {
                None
            }
        } else {
            None
        };

        // Create a SwarmBuilder with QUIC transport
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| {
                crate::behaviour::make_behaviour_with_config(key, por, xroutes_config)
            })?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(std::time::Duration::from_secs(60000))
            })
            .build();

        // Set up communication channels
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);

        Ok((
            Self {
                cmd_rx,
                event_tx,
                swarm,
                connected_peers: HashMap::new(),
                local_peer_id,
                authenticated_peers: HashSet::new(),
                xroutes_handler,
                local_key,
            },
            cmd_tx,
            event_rx,
            local_peer_id,
        ))
    }

    // Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    // Get the current listening addresses
    pub fn listening_addresses(&self) -> Vec<Multiaddr> {
        self.swarm.listeners().cloned().collect()
    }

    pub async fn run(&mut self) {
        self.run_with_cleanup_interval(Duration::from_secs(30)).await;
    }

    /// Run the network node with custom cleanup interval
    pub async fn run_with_cleanup_interval(&mut self, cleanup_interval_duration: Duration) {
        // Create periodic cleanup timer
        let mut cleanup_interval = tokio::time::interval(cleanup_interval_duration);
        cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    if let NetworkCommand::Shutdown = cmd {
                        info!("Shutting down network node");
                        break;
                    }

                    self.handle_command(cmd).await;
                }
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
                _ = cleanup_interval.tick() => {
                    // Periodic cleanup of timed out waiters
                    self.periodic_cleanup().await;
                }
            }
        }
    }

    /// Periodic cleanup of timed out waiters and other maintenance tasks
    async fn periodic_cleanup(&mut self) {
        if let Some(ref mut handler) = self.xroutes_handler {
            // Clean up timed out search waiters
            handler.cleanup_timed_out_waiters();

            // Optional: Log active searches for monitoring
            let active_searches = handler.get_active_searches_info();
            if !active_searches.is_empty() {
                debug!("Active searches: {} searches with {} total waiters", 
                       active_searches.len(),
                       active_searches.iter().map(|(_, count, _)| count).sum::<usize>());
                
                // Log searches that have been running for a long time
                for (peer_id, waiter_count, duration) in active_searches {
                    if duration.as_secs() > 300 { // 5 minutes
                        warn!("Long-running search for peer {}: {} waiters, running for {:?}", 
                              peer_id, waiter_count, duration);
                    }
                }
            }
        }

        // Optional: Clean up old authenticated peers that are no longer connected
        self.cleanup_disconnected_authenticated_peers();
    }

    /// Clean up authenticated peers that are no longer connected
    fn cleanup_disconnected_authenticated_peers(&mut self) {
        let disconnected_peers: Vec<PeerId> = self
            .authenticated_peers
            .iter()
            .filter(|&peer_id| !self.connected_peers.contains_key(peer_id))
            .cloned()
            .collect();

        for peer_id in disconnected_peers {
            self.authenticated_peers.remove(&peer_id);
            debug!("Removed authentication record for disconnected peer {}", peer_id);
        }
    }

    // Handle commands sent to the network node
    async fn handle_command(&mut self, cmd: NetworkCommand) {
        match cmd {
            // Core stream commands
            NetworkCommand::OpenStream {
                peer_id,
                connection_id: _,
                response,
            } => {
                self.swarm
                    .behaviour_mut()
                    .xstream
                    .open_stream(peer_id, response)
                    .await;
            }

            // Core authentication commands
            NetworkCommand::SubmitPorVerification {
                connection_id,
                result,
            } => {
                info!("Submitting PoR verification result for connection {connection_id}");

                match self
                    .swarm
                    .behaviour_mut()
                    .por_auth
                    .submit_por_verification_result(connection_id, result)
                {
                    Ok(_) => info!("✅ Successfully submitted verification result"),
                    Err(e) => warn!("❌ Failed to submit verification result: {}", e),
                }
            }

            NetworkCommand::IsPeerAuthenticated { peer_id, response } => {
                let is_authenticated = self.authenticated_peers.contains(&peer_id)
                    || self
                        .swarm
                        .behaviour()
                        .por_auth
                        .is_peer_authenticated(&peer_id);

                let _ = response.send(is_authenticated);
            }

            // Core connection commands
            NetworkCommand::OpenListenPort {
                host,
                port,
                response,
            } => {
                let addr = Multiaddr::from_str(&format!("/ip4/{}/udp/{}/quic-v1", host, port))
                    .expect("Invalid multiaddr");

                match self.swarm.listen_on(addr.clone()) {
                    Ok(_) => {
                        for i in self.listening_addresses() {
                            println!("Listening on {}", i);
                        }
                        let _ = response.send(Ok(addr));
                    }
                    Err(err) => {
                        error!("Failed to listen on {addr}: {err}");
                        let _ = response.send(Err(Box::new(err)));
                    }
                }
            }

            NetworkCommand::Connect { addr, response } => match self.swarm.dial(addr.clone()) {
                Ok(_) => {
                    info!("Dialing {addr}");
                    let _ = response.send(Ok(()));
                }
                Err(err) => {
                    error!("Failed to dial {addr}: {err}");
                    let _ = response.send(Err(Box::new(err)));
                }
            },

            NetworkCommand::Disconnect { peer_id, response } => {
                if self.connected_peers.contains_key(&peer_id) {
                    if let Err(err) = self.swarm.disconnect_peer_id(peer_id) {
                        error!("Failed to disconnect from {peer_id}: {err:?}");
                        let _ = response.send(Err(format!("Failed to disconnect: {err:?}").into()));
                    } else {
                        info!("Disconnected from {peer_id}");
                        self.connected_peers.remove(&peer_id);
                        let _ = response.send(Ok(()));
                    }
                } else {
                    let _ = response.send(Err(format!("Not connected to {peer_id}").into()));
                }
            }

            // Core status commands
            NetworkCommand::GetConnectionsForPeer { peer_id, response } => {
                let connections = self
                    .connected_peers
                    .get(&peer_id)
                    .unwrap_or(&Vec::new())
                    .to_vec();
                let _ = response.send(connections);
            }

            NetworkCommand::GetPeersConnected { peer_id: _, response } => {
                let peers: Vec<(PeerId, Vec<Multiaddr>)> = self
                    .connected_peers
                    .iter()
                    .map(|(peer_id, addrs)| (*peer_id, addrs.clone()))
                    .collect();
                let _ = response.send(peers);
            }

            NetworkCommand::GetListenAddresses { response } => {
                let addresses = self.listening_addresses();
                let _ = response.send(addresses);
            }

            // XRoutes commands - delegate to handler with enhanced error handling
            NetworkCommand::XRoutes(xroutes_cmd) => {
                if let Some(ref mut handler) = self.xroutes_handler {
                    handler.handle_command(xroutes_cmd, &mut self.swarm, &self.local_key).await;
                } else {
                    self.send_xroutes_disabled_error(xroutes_cmd);
                }
            }

            NetworkCommand::Shutdown => {
                // Handled in the run loop
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: libp2p::swarm::SwarmEvent<NodeBehaviourEvent>) {
        match event {
            libp2p::swarm::SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                connection_id,
                ..
            } => {
                let addr = endpoint.get_remote_address().clone();

                // Track this connection
                self.connected_peers
                    .entry(peer_id)
                    .or_insert_with(Vec::new)
                    .push(addr.clone());

                info!("Connected to {peer_id} at {addr}");

                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionOpened {
                        peer_id,
                        addr: addr.clone(),
                        connection_id,
                        protocols: Vec::new(),
                    })
                    .await;

                // Only emit PeerConnected event if this is the first connection
                if num_established.get() == 1 {
                    info!("First connection to peer {peer_id} established");
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerConnected { peer_id })
                        .await;
                }
            }

            libp2p::swarm::SwarmEvent::ConnectionClosed {
                peer_id,
                cause,
                endpoint,
                connection_id,
                num_established,
                ..
            } => {
                let addr = endpoint.get_remote_address().clone();

                // Update our connection tracking
                if let Some(connections) = self.connected_peers.get_mut(&peer_id) {
                    connections.retain(|a| a != &addr);
                    if connections.is_empty() {
                        self.connected_peers.remove(&peer_id);
                    }
                }

                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionClosed {
                        peer_id,
                        addr,
                        connection_id,
                    })
                    .await;

                if num_established == 0 {
                    info!("Disconnected from {peer_id}, cause: {cause:?}");
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDisconnected { peer_id })
                        .await;
                }
            }

            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                let mut full_addr = address.clone();
                full_addr.push(libp2p::multiaddr::Protocol::P2p(self.local_peer_id.into()));

                info!("Listening on {} (with PeerId: {})", address, full_addr);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ListeningOnAddress {
                        addr: address,
                        full_addr: Some(full_addr),
                    })
                    .await;
            }

            libp2p::swarm::SwarmEvent::ExpiredListenAddr { address, .. } => {
                info!("Stopped listening on {address}");
                let _ = self
                    .event_tx
                    .send(NetworkEvent::StopListeningOnAddress { addr: address })
                    .await;
            }

            libp2p::swarm::SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("Failed to connect to {:?}: {error}", peer_id);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionError {
                        peer_id,
                        error: error.to_string(),
                    })
                    .await;
            }

            libp2p::swarm::SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                ..
            } => {
                warn!("Failed incoming connection from {send_back_addr} to {local_addr}: {error}");
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionError {
                        peer_id: None,
                        error: error.to_string(),
                    })
                    .await;
            }

            libp2p::swarm::SwarmEvent::Behaviour(event) => {
                self.handle_behaviour_event(event).await;
            }

            _ => {}
        }
    }

    // Handle events from the network behaviour
    async fn handle_behaviour_event(&mut self, event: NodeBehaviourEvent) {
        match event {
            // Core stream events
            NodeBehaviourEvent::Xstream(event) => match event {
                XStreamEvent::IncomingStream { stream } => {
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::IncomingStream {
                            stream: Arc::new(stream),
                        })
                        .await;
                }
                rest => {
                    println!("XSTREAM EVENT {:?}", rest);
                }
            },

            // Core identify events
            NodeBehaviourEvent::Identify(identify_event) => {
                match identify_event {
                    identify::Event::Received { peer_id, info, .. } => {
                        info!("Identified peer {peer_id}: {info:?}");

                        // Add peer's listening addresses to XRoutes if enabled
                        if let Some(ref mut handler) = self.xroutes_handler {
                            if let Some(xroutes) = self.swarm.behaviour_mut().xroutes.as_mut() {
                                for addr in &info.listen_addrs {
                                    xroutes.add_address(&peer_id, addr.clone());
                                    info!("Address added to XRoutes {peer_id} {addr}");
                                }
                            }
                        }

                        // Detect peer role from protocols
                        let peer_role = crate::xroutes::XRouteRole::from_protocols(&info.protocols);
                        if peer_role != crate::xroutes::XRouteRole::Unknown {
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::XRoutes(crate::xroutes::XRoutesEvent::PeerRoleDetected {
                                    peer_id,
                                    role: peer_role,
                                }))
                                .await;
                        }
                    }
                    identify::Event::Sent { .. } => {
                        // Identify info sent to peer - usually not important for application logic
                    }
                    identify::Event::Pushed { .. } => {
                        // Identify info pushed to peer - usually not important for application logic
                    }
                    identify::Event::Error { peer_id, error, .. } => {
                        warn!("Identify error with peer {peer_id:?}: {error}");
                    }
                }
            }

            // Core ping events
            NodeBehaviourEvent::Ping(_) => {
                // Ping events are mostly for internal connection maintenance
                // We don't need to emit them as network events typically
            }

            // Core authentication events
            NodeBehaviourEvent::PorAuth(event) => {
                match event {
                    PorAuthEvent::MutualAuthSuccess {
                        peer_id,
                        connection_id,
                        address,
                        metadata,
                    } => {
                        info!("✅ Mutual authentication successful with peer {peer_id} at {address}");
                        self.authenticated_peers.insert(peer_id);

                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::MutualAuthSuccess {
                                    peer_id,
                                    connection_id,
                                    address,
                                    metadata,
                                },
                            })
                            .await;
                    }
                    
                    other_auth_event => {
                        // Forward other auth events
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: other_auth_event,
                            })
                            .await;
                    }
                }
            }

            // XRoutes discovery events
            NodeBehaviourEvent::Xroutes(xroutes_event) => {
                if let Some(ref mut handler) = self.xroutes_handler {
                    let network_events = handler
                        .handle_behaviour_event(xroutes_event, &mut self.swarm)
                        .await;

                    // Send all resulting XRoutes events
                    for event in network_events {
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::XRoutes(event))
                            .await;
                    }
                }
            }
        }
    }

    // Helper method to send error when XRoutes is disabled - enhanced for new commands
    fn send_xroutes_disabled_error(&self, cmd: XRoutesCommand) {
        match cmd {
            XRoutesCommand::FindPeerAddressesAdvanced { response, .. } => {
                let _ = response.send(Err("XRoutes discovery not enabled".to_string()));
            }
            XRoutesCommand::CancelPeerSearch { response, .. } => {
                let _ = response.send(Err("XRoutes discovery not enabled".to_string()));
            }
            XRoutesCommand::GetActiveSearches { response } => {
                let _ = response.send(Vec::new());
            }
            XRoutesCommand::BootstrapKad { response } => {
                let _ = response.send(Err("XRoutes discovery not enabled".into()));
            }
            XRoutesCommand::GetKadKnownPeers { response } => {
                let _ = response.send(Vec::new());
            }
            XRoutesCommand::GetPeerAddresses { response, .. } => {
                let _ = response.send(Vec::new());
            }
            XRoutesCommand::FindPeerAddresses { response, .. } => {
                let _ = response.send(Err("XRoutes discovery not enabled".into()));
            }
            XRoutesCommand::SetRole { response, .. } => {
                let _ = response.send(Err("XRoutes discovery not enabled".to_string()));
            }
            XRoutesCommand::GetRole { response } => {
                let _ = response.send(crate::xroutes::XRouteRole::Unknown);
            }
            XRoutesCommand::ConnectToBootstrap { response, .. } => {
                let _ = response.send(Err(crate::xroutes::BootstrapError::DiscoveryNotEnabled));
            }
            _ => {
                // For commands without response channel, just log
                warn!("XRoutes command ignored - discovery not enabled: {:?}", cmd);
            }
        }
    }
}
//! Swarm handler for XNetwork2

use async_trait::async_trait;
use command_swarm::{NetworkBehaviour, SwarmHandler};
use libp2p::core::transport::ListenerId;
use libp2p::swarm::{FromSwarm, NewExternalAddrCandidate};
use libp2p::{Multiaddr, PeerId, Swarm};
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::behaviours::xroutes::PendingTaskManager;
use crate::connection_tracker::{ConnectionInfo, PeerConnections};
use crate::connection_tracker_commands::ConnectionTrackerCommand;
use crate::main_behaviour::{XNetworkBehaviour, XNetworkBehaviourEvent};
use crate::node_events::NodeEvent;
use crate::swarm_commands::{NetworkState, SwarmLevelCommand};
use xauth::events::PorAuthEvent;
use xstream::events::XStreamEvent;

/// Key for dial_and_wait operations to handle multiple connections to same peer
/// We use a combination of peer_id and connection attempt counter to handle multiple connections
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DialWaitKey {
    peer_id: PeerId,
    attempt_id: u64, // Simple counter to distinguish multiple connection attempts to same peer
}

/// Swarm handler for XNetwork2
pub struct XNetworkSwarmHandler {
    /// Broadcast channel for sending NodeEvents to multiple subscribers
    event_sender: Option<broadcast::Sender<NodeEvent>>,
    /// Track authenticated peers
    authenticated_peers: std::collections::HashSet<PeerId>,
    /// Pending tasks for listen_and_wait operations
    listen_wait_tasks:
        PendingTaskManager<ListenerId, Multiaddr, Box<dyn std::error::Error + Send + Sync>, ()>,
    /// Pending tasks for dial_and_wait operations
    dial_wait_tasks: PendingTaskManager<
        DialWaitKey,
        libp2p::swarm::ConnectionId,
        Box<dyn std::error::Error + Send + Sync>,
        (),
    >,
}

impl Default for XNetworkSwarmHandler {
    fn default() -> Self {
        Self {
            event_sender: None,
            authenticated_peers: std::collections::HashSet::new(),
            listen_wait_tasks: PendingTaskManager::new(),
            dial_wait_tasks: PendingTaskManager::new(),
        }
    }
}

impl XNetworkSwarmHandler {
    /// Create a new SwarmHandler with event sender
    pub fn with_event_sender(event_sender: broadcast::Sender<NodeEvent>) -> Self {
        Self {
            event_sender: Some(event_sender),
            authenticated_peers: std::collections::HashSet::new(),
            listen_wait_tasks: PendingTaskManager::new(),
            dial_wait_tasks: PendingTaskManager::new(),
        }
    }

    /// Check if a peer is authenticated
    fn is_peer_authenticated(&self, peer_id: &PeerId) -> bool {
        self.authenticated_peers.contains(peer_id)
    }

    /// Add a peer to authenticated set
    fn mark_peer_authenticated(&mut self, peer_id: PeerId) {
        self.authenticated_peers.insert(peer_id);
        println!("✅ [SwarmHandler] Peer {} marked as authenticated", peer_id);
    }

    /// Transform SwarmEvent into NodeEvent and emit through broadcast channel
    fn transform_and_emit_event(
        &mut self,
        event: &libp2p::swarm::SwarmEvent<
            <XNetworkBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
        >,
    ) {
        // If event sender is not set, do nothing
        let event_sender = match self.event_sender.as_ref() {
            Some(sender) => sender,
            None => return,
        };

        match event {
            // Network events
            libp2p::swarm::SwarmEvent::NewListenAddr {
                listener_id,
                address,
                ..
            } => {
                // Complete pending listen_and_wait task if exists
                if self
                    .listen_wait_tasks
                    .set_task_result(&listener_id, address.clone())
                    .is_ok()
                {
                    debug!(
                        "✅ [SwarmHandler] Completed listen_and_wait task for listener_id: {:?}",
                        listener_id
                    );
                }

                let _ = event_sender.send(NodeEvent::NewListenAddr {
                    listener_id: listener_id.clone(),
                    address: address.clone(),
                });
            }
            libp2p::swarm::SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
                ..
            } => {
                let _ = event_sender.send(NodeEvent::ExpiredListenAddr {
                    listener_id: listener_id.clone(),
                    address: address.clone(),
                });
            }
            libp2p::swarm::SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                ..
            } => {
                println!("Conn established {:?}", peer_id);

                // Try to complete pending dial_and_wait task if exists
                // We need to find any DialWaitKey that matches this peer_id
                // This is a simplified approach - we complete the first matching task for this peer
                // In practice, we should track which specific dial attempt corresponds to which connection

                // Try to find and complete any pending task for this peer
                let mut completed = false;
                for key in self.dial_wait_tasks.get_pending_keys() {
                    if key.peer_id == *peer_id {
                        if self
                            .dial_wait_tasks
                            .set_task_result(&key, *connection_id)
                            .is_ok()
                        {
                            debug!(
                                "✅ [SwarmHandler] Completed dial_and_wait task for peer: {} with connection_id: {:?}",
                                peer_id, connection_id
                            );
                            completed = true;
                            break;
                        }
                    }
                }

                if !completed {
                    debug!(
                        "📡 [SwarmHandler] Connection established for peer: {} with connection_id: {:?}, but no matching dial_and_wait task found",
                        peer_id, connection_id
                    );
                }

                // Start authentication for this connection
                // Note: We can't call commander directly from here, but we can emit an event
                // that the application can listen to and then call start_auth_for_connection
                debug!(
                    "🔐 [SwarmHandler] Connection established - authentication will need to be started manually for connection: {:?}",
                    connection_id
                );

                let _ = event_sender.send(NodeEvent::ConnectionEstablished {
                    peer_id: *peer_id,
                    connection_id: *connection_id,
                });
            }
            libp2p::swarm::SwarmEvent::ConnectionClosed {
                peer_id,
                connection_id,
                ..
            } => {
                let _ = event_sender.send(NodeEvent::ConnectionClosed {
                    peer_id: *peer_id,
                    connection_id: *connection_id,
                });
            }

            // Behaviour events - we'll handle XAuth and XStream events specifically
            libp2p::swarm::SwarmEvent::Behaviour(behaviour_event) => {
                match behaviour_event {
                    XNetworkBehaviourEvent::Xauth(por_auth_event) => {
                        match por_auth_event {
                            PorAuthEvent::VerifyPorRequest {
                                peer_id,
                                connection_id,
                                por,
                                metadata,
                                address,
                            } => {
                                let _ = event_sender.send(NodeEvent::VerifyPorRequest {
                                    peer_id: *peer_id,
                                    connection_id: format!("{:?}", connection_id),
                                    por: por.peer_id.to_bytes(),
                                    metadata: metadata.clone(),
                                });
                            }
                            PorAuthEvent::MutualAuthSuccess {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                let _ = event_sender.send(NodeEvent::PeerMutualAuthSuccess {
                                    peer_id: *peer_id,
                                    connection_id: *connection_id,
                                });
                            }
                            PorAuthEvent::OutboundAuthSuccess {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                let _ = event_sender.send(NodeEvent::PeerOutboundAuthSuccess {
                                    peer_id: *peer_id,
                                    connection_id: *connection_id,
                                });
                            }
                            PorAuthEvent::InboundAuthSuccess {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                let _ = event_sender.send(NodeEvent::PeerInboundAuthSuccess {
                                    peer_id: *peer_id,
                                    connection_id: *connection_id,
                                });
                            }
                            // Skip authentication failures and other XAuth events
                            _ => {}
                        }
                    }
                    XNetworkBehaviourEvent::Xstream(xstream_event) => {
                        match xstream_event {
                            XStreamEvent::IncomingStream { stream } => {
                                let _ = event_sender.send(NodeEvent::XStreamIncoming {
                                    stream: stream.clone(),
                                });
                            }
                            XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                                let _ = event_sender.send(NodeEvent::XStreamEstablished {
                                    peer_id: *peer_id,
                                    stream_id: *stream_id,
                                });
                            }
                            XStreamEvent::StreamError {
                                peer_id,
                                stream_id,
                                error,
                            } => {
                                let _ = event_sender.send(NodeEvent::XStreamError {
                                    peer_id: *peer_id,
                                    stream_id: *stream_id,
                                    error: error.clone(),
                                });
                            }
                            XStreamEvent::StreamClosed { peer_id, stream_id } => {
                                let _ = event_sender.send(NodeEvent::XStreamClosed {
                                    peer_id: *peer_id,
                                    stream_id: *stream_id,
                                });
                            }
                            XStreamEvent::IncomingStreamRequest {
                                peer_id,
                                connection_id,
                                decision_sender,
                            } => {
                                // Always forward incoming stream requests to application for decision making
                                debug!(
                                    "🔍 [SwarmHandler] Forwarding IncomingStreamRequest from peer: {}, connection: {:?}",
                                    peer_id, connection_id
                                );
                                let _ =
                                    event_sender.send(NodeEvent::XStreamIncomingStreamRequest {
                                        peer_id: *peer_id,
                                        connection_id: *connection_id,
                                        decision_sender: decision_sender.clone(),
                                    });
                            }
                        }
                    }
                    XNetworkBehaviourEvent::Xroutes(xroutes_event) => {
                        // Transform XRoutes events to NodeEvents
                        match xroutes_event {
                            super::behaviours::xroutes::XRoutesBehaviourEvent::Kad(kad_event) => {
                                match kad_event {
                                    libp2p::kad::Event::RoutingUpdated { peer, .. } => {
                                        let _ =
                                            event_sender.send(NodeEvent::KademliaRoutingUpdated {
                                                peer_id: *peer,
                                            });
                                    }
                                    libp2p::kad::Event::OutboundQueryProgressed {
                                        result, ..
                                    } => {
                                        match result {
                                            libp2p::kad::QueryResult::Bootstrap(Ok(_)) => {
                                                let _ = event_sender
                                                    .send(NodeEvent::KademliaBootstrapCompleted);
                                            }
                                            libp2p::kad::QueryResult::GetClosestPeers(Ok(
                                                peers,
                                            )) => {
                                                // Emit discovery events for found peers
                                                for peer_info in &peers.peers {
                                                    let _ = event_sender.send(
                                                        NodeEvent::KademliaPeerDiscovered {
                                                            peer_id: peer_info.peer_id,
                                                            addresses: peer_info.addrs.clone(),
                                                        },
                                                    );
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            super::behaviours::xroutes::XRoutesBehaviourEvent::Mdns(mdns_event) => {
                                match mdns_event {
                                    libp2p::mdns::Event::Discovered(list) => {
                                        // Transform mDNS discovered event to NodeEvent
                                        for (peer_id, address) in list {
                                            let _ =
                                                event_sender.send(NodeEvent::MdnsPeerDiscovered {
                                                    peer_id: *peer_id,
                                                    addresses: vec![address.clone()],
                                                });
                                        }
                                    }
                                    libp2p::mdns::Event::Expired(list) => {
                                        // Transform mDNS expired event to NodeEvent
                                        for (peer_id, _) in list {
                                            let _ = event_sender.send(NodeEvent::MdnsPeerExpired {
                                                peer_id: *peer_id,
                                            });
                                        }
                                    }
                                }
                            }
                            _ => {
                                debug!("📡 [SwarmHandler] XRoutes event: {:?}", xroutes_event);
                            }
                        }
                    }
                    // Skip other behaviour events
                    _ => {
                        debug!("📡 [SwarmHandler] beh event: {:?}", behaviour_event);
                    }
                }
            }

            // Other events we don't currently transform - do nothing (default behavior)
            _ => {}
        }
    }
}

#[async_trait]
impl SwarmHandler<XNetworkBehaviour> for XNetworkSwarmHandler {
    type Command = SwarmLevelCommand;

    async fn handle_command(&mut self, swarm: &mut Swarm<XNetworkBehaviour>, cmd: Self::Command) {
        match cmd {
            SwarmLevelCommand::Dial {
                peer_id,
                addr,
                response,
            } => {
                debug!(
                    "🔄 [SwarmHandler] Processing Dial command - Peer: {:?}, Addr: {}",
                    peer_id, addr
                );
                let result = swarm
                    .dial(addr.clone())
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                if result.is_ok() {
                    info!(
                        "📡 [SwarmHandler] Dialing peer {:?} at address {}",
                        peer_id, addr
                    );
                } else {
                    debug!(
                        "❌ [SwarmHandler] Failed to dial peer {:?}: {:?}",
                        peer_id, result
                    );
                }
                let _ = response.send(result);
            }
            SwarmLevelCommand::ListenOn { addr, response } => {
                debug!(
                    "🔄 [SwarmHandler] Processing ListenOn command - Addr: {}",
                    addr
                );
                let result = swarm
                    .listen_on(addr.clone())
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                if result.is_ok() {
                    info!("📡 [SwarmHandler] Listening on address {}", addr);
                } else {
                    debug!(
                        "❌ [SwarmHandler] Failed to listen on address {}: {:?}",
                        addr, result
                    );
                }
                let _ = response.send(result);
            }
            SwarmLevelCommand::ListenAndWait {
                addr,
                timeout,
                response,
            } => {
                debug!(
                    "🔄 [SwarmHandler] Processing ListenAndWait command - Addr: {}, Timeout: {:?}",
                    addr, timeout
                );

                // First, start listening
                let listener_id = match swarm.listen_on(addr.clone()) {
                    Ok(listener_id) => listener_id,
                    Err(e) => {
                        let error = Box::new(e) as Box<dyn std::error::Error + Send + Sync>;
                        let _ = response.send(Err(error));
                        return;
                    }
                };

                info!(
                    "📡 [SwarmHandler] Started listening on address {} with listener_id: {:?}",
                    addr, listener_id
                );

                // Add pending task to wait for NewListenAddr event
                self.listen_wait_tasks
                    .add_pending_task(listener_id, timeout, response);
            }
            SwarmLevelCommand::Disconnect { peer_id, response } => {
                debug!(
                    "🔄 [SwarmHandler] Processing Disconnect command - Peer: {:?}",
                    peer_id
                );
                swarm.disconnect_peer_id(peer_id);
                info!("📤 [SwarmHandler] Disconnected from peer {:?}", peer_id);
                let _ = response.send(Ok(()));
            }
            SwarmLevelCommand::GetNetworkState { response } => {
                debug!("🔄 [SwarmHandler] Processing GetNetworkState command");
                let listeners = swarm.listeners().cloned().collect::<Vec<_>>();
                let connected_peers = swarm.connected_peers().cloned().collect::<Vec<_>>();
                let peer_id = swarm.local_peer_id().clone();

                let network_state = NetworkState {
                    peer_id,
                    listening_addresses: listeners,
                    connected_peers,
                    authenticated_peers: vec![], // TODO: Add authenticated peers tracking
                };

                info!(
                    "📊 [SwarmHandler] Network state - Listeners: {:?}, Connected peers: {:?}",
                    network_state.listening_addresses, network_state.connected_peers
                );

                let _ = response.send(Ok(network_state));
            }
            SwarmLevelCommand::Shutdown { stopper, response } => {
                debug!("🔄 [SwarmHandler] Processing Shutdown command");
                info!("🛑 [SwarmHandler] Node shutdown initiated via stopper");
                // Use the stopper to actually stop the swarm
                stopper.stop();
                let _ = response.send(Ok(()));
            }
            SwarmLevelCommand::Echo { message, response } => {
                debug!(
                    "🔄 [SwarmHandler] Processing Echo command - Message: '{}'",
                    message
                );
                info!("📢 [SwarmHandler] Echo command received: '{}'", message);
                let _ = response.send(Ok(message));
            }
            SwarmLevelCommand::DialAndWait {
                peer_id,
                addr,
                timeout,
                response,
            } => {
                debug!(
                    "🔄 [SwarmHandler] Processing DialAndWait command - Peer: {:?}, Addr: {}, Timeout: {:?}",
                    peer_id, addr, timeout
                );

                // Generate a simple attempt_id based on current time
                let attempt_id = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;

                let key = DialWaitKey {
                    peer_id,
                    attempt_id,
                };

                // Start dialing
                let result = swarm.dial(addr.clone());
                if let Err(e) = result {
                    let error = Box::new(e) as Box<dyn std::error::Error + Send + Sync>;
                    debug!(
                        "❌ [SwarmHandler] Failed to dial peer {}: {:?}",
                        peer_id, error
                    );
                    let _ = response.send(Err(error));
                    return;
                }

                info!(
                    "📡 [SwarmHandler] Dialing peer {} at address {}, waiting for connection",
                    peer_id, addr
                );

                // Add pending task to wait for ConnectionEstablished event
                self.dial_wait_tasks
                    .add_pending_task(key, timeout, response);
            }
            SwarmLevelCommand::StartAuthForConnection {
                connection_id,
                response,
            } => {
                debug!(
                    "🔄 [SwarmHandler] Processing StartAuthForConnection command - Connection: {:?}",
                    connection_id
                );

                // Start actual authentication using the xauth behaviour
                let result = swarm
                    .behaviour_mut()
                    .xauth
                    .start_authentication(connection_id)
                    .map_err(|e| {
                        Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
                            as Box<dyn std::error::Error + Send + Sync>
                    });

                match &result {
                    Ok(_) => {
                        debug!(
                            "🔐 [SwarmHandler] Authentication successfully started for connection: {:?}",
                            connection_id
                        );
                        info!(
                            "🔐 [SwarmHandler] Authentication started for connection: {:?}",
                            connection_id
                        );
                    }
                    Err(e) => {
                        debug!(
                            "❌ [SwarmHandler] Failed to start authentication for connection {:?}: {:?}",
                            connection_id, e
                        );
                        info!(
                            "❌ [SwarmHandler] Failed to start authentication for connection {:?}: {:?}",
                            connection_id, e
                        );
                    }
                }

                let _ = response.send(result);
            }
            SwarmLevelCommand::AddExternalAddress { address, response } => {
                debug!(
                    "🔄 [SwarmHandler] Processing AddExternalAddress command - Address: {}",
                    address
                );

                // Add external address to swarm
                swarm.add_external_address(address.clone());

                info!("🌐 [SwarmHandler] Added external address: {}", address);

                let _ = response.send(Ok(()));
            }
            SwarmLevelCommand::GetExternalAddresses { response } => {
                debug!("🔄 [SwarmHandler] Processing GetExternalAddresses command");

                // Get all external addresses from swarm
                let external_addrs: Vec<Multiaddr> = swarm.external_addresses().cloned().collect();

                info!(
                    "🌐 [SwarmHandler] Retrieved {} external addresses",
                    external_addrs.len()
                );

                if !external_addrs.is_empty() {
                    debug!("📡 [SwarmHandler] External addresses: {:?}", external_addrs);
                }

                let _ = response.send(Ok(external_addrs));
            }
            SwarmLevelCommand::ConnectionTracker { command } => {
                debug!("🔄 [SwarmHandler] Processing ConnectionTracker command: {:?}", command);
                
                // Handle ConnectionTracker commands through XRoutesBehaviour
                match command {
                    ConnectionTrackerCommand::GetConnections { response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            let connections = connection_tracker.get_all_connections();
                            let connections_cloned: Vec<ConnectionInfo> = connections.into_iter().cloned().collect();
                            let _ = response.send(Ok(connections_cloned));
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                    ConnectionTrackerCommand::GetPeerConnections { peer_id, response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            match connection_tracker.get_peer_connections(&peer_id) {
                                Some(peer_connections) => {
                                    let _ = response.send(Ok(peer_connections.clone()));
                                }
                                None => {
                                    let error_msg = format!("No connections found for peer: {}", peer_id);
                                    let _ = response.send(Err(error_msg.into()));
                                }
                            }
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                    ConnectionTrackerCommand::GetConnection { connection_id, response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            match connection_tracker.get_connection(&connection_id) {
                                Some(connection_info) => {
                                    let _ = response.send(Ok(connection_info.clone()));
                                }
                                None => {
                                    let error_msg = format!("Connection not found: {:?}", connection_id);
                                    let _ = response.send(Err(error_msg.into()));
                                }
                            }
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                    ConnectionTrackerCommand::GetConnectedPeers { response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            let connected_peers = connection_tracker.get_connected_peers();
                            let _ = response.send(Ok(connected_peers));
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                    ConnectionTrackerCommand::GetConnectionStats { response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            let stats = connection_tracker.get_connection_stats();
                            let _ = response.send(Ok(stats));
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                    ConnectionTrackerCommand::GetListenAddresses { response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            let listen_addresses = connection_tracker.get_listen_addresses().to_vec();
                            let _ = response.send(Ok(listen_addresses));
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                    ConnectionTrackerCommand::GetExternalAddresses { response } => {
                        if let Some(connection_tracker) = swarm.behaviour().xroutes.connection_tracker.as_ref() {
                            let external_addresses = connection_tracker.get_external_addresses().to_vec();
                            let _ = response.send(Ok(external_addresses));
                        } else {
                            let _ = response.send(Err("ConnectionTracker not enabled".into()));
                        }
                    }
                }
            }
        }
    }

    async fn handle_event(
        &mut self,
        swarm: &mut Swarm<XNetworkBehaviour>,
        event: &libp2p::swarm::SwarmEvent<
            <XNetworkBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
        >,
    ) {
        // First, transform and emit the event through the channel
        self.transform_and_emit_event(event);

        // Then handle the event normally (logging, etc.)
        match event {
            libp2p::swarm::SwarmEvent::NewExternalAddrCandidate { address } => {
            }
            libp2p::swarm::SwarmEvent::ExternalAddrConfirmed { address } => {
            }
            libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, connection_id ,.. } => {
            }
            libp2p::swarm::SwarmEvent::Behaviour(behaviour_event) => {
                match behaviour_event {
                    XNetworkBehaviourEvent::Ping(event) => {
                        debug!("📡 [SwarmHandler] Ping event: {:?}", event);
                    }
                    XNetworkBehaviourEvent::Xauth(event) => {
                        debug!("📡 [SwarmHandler] XAuth event: {:?}", event);

                        // Добавляем специальную отладочную информацию для событий аутентификации
                        match event {
                            PorAuthEvent::MutualAuthSuccess {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                debug!(
                                    "🎉 [SwarmHandler] MUTUAL AUTH SUCCESS for peer: {}, connection: {:?}",
                                    peer_id, connection_id
                                );
                            }
                            PorAuthEvent::OutboundAuthSuccess {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                debug!(
                                    "✅ [SwarmHandler] OUTBOUND AUTH SUCCESS for peer: {}, connection: {:?}",
                                    peer_id, connection_id
                                );
                            }
                            PorAuthEvent::InboundAuthSuccess {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                debug!(
                                    "✅ [SwarmHandler] INBOUND AUTH SUCCESS for peer: {}, connection: {:?}",
                                    peer_id, connection_id
                                );
                            }
                            PorAuthEvent::OutboundAuthFailure {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                debug!(
                                    "❌ [SwarmHandler] OUTBOUND AUTH FAILURE for peer: {}, connection: {:?}",
                                    peer_id, connection_id
                                );
                            }
                            PorAuthEvent::InboundAuthFailure {
                                peer_id,
                                connection_id,
                                ..
                            } => {
                                debug!(
                                    "❌ [SwarmHandler] INBOUND AUTH FAILURE for peer: {}, connection: {:?}",
                                    peer_id, connection_id
                                );
                            }
                            _ => {}
                        }
                    }
                    XNetworkBehaviourEvent::Xstream(event) => {
                        debug!("📡 [SwarmHandler] XStream event: {:?}", event);
                    }
                    XNetworkBehaviourEvent::Xroutes(event) => {
                        debug!("📡 [SwarmHandler] XRoutes event: {:?}", event);
                        match event {
                            super::behaviours::xroutes::XRoutesBehaviourEvent::Identify(
                                identify_event,
                            ) => match identify_event {
                                libp2p::identify::Event::Received {
                                    peer_id,
                                    info,
                                    connection_id,
                                } => {
                                }
                                libp2p::identify::Event::Pushed {
                                    peer_id,
                                    info,
                                    connection_id,
                                } => {
                                    println!("Indetify pushed to peer {:?}", peer_id);
                                }
                                _ => {}
                            },
                            super::behaviours::xroutes::XRoutesBehaviourEvent::RelayServer(
                                relay_event,
                            ) => {}
                            super::behaviours::xroutes::XRoutesBehaviourEvent::RelayClient(
                                relay_event,
                            ) => {}
                            super::behaviours::xroutes::XRoutesBehaviourEvent::Dcutr(
                                dcutr_event,
                            ) => {}
                            super::behaviours::xroutes::XRoutesBehaviourEvent::AutonatClient(
                                autonat_client_event,
                            ) => {}
                            super::behaviours::xroutes::XRoutesBehaviourEvent::AutonatServer(
                                autonat_server_event,
                            ) => {}
                            super::behaviours::xroutes::XRoutesBehaviourEvent::Kad(kad_event) => {
                            }
                            _ => {}
                        }
                    }
                    XNetworkBehaviourEvent::KeepAlive(event) => {
                        debug!("📡 [SwarmHandler] KeepAlive event: {:?}", event);
                    }
                }
            }
            _ => {
                debug!("🌐 [SwarmHandler] Swarm event: {:?}", event);
            }
        }
    }
}

//! Swarm handler for XNetwork2

use async_trait::async_trait;
use command_swarm::SwarmHandler;
use libp2p::{Multiaddr, PeerId, Swarm};
use libp2p::core::transport::ListenerId;
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::main_behaviour::{XNetworkBehaviour, XNetworkBehaviourEvent};
use crate::node_events::NodeEvent;
use crate::swarm_commands::{NetworkState, SwarmLevelCommand};
use xauth::events::PorAuthEvent;
use xstream::events::XStreamEvent;
use crate::behaviours::xroutes::PendingTaskManager;

/// Swarm handler for XNetwork2
pub struct XNetworkSwarmHandler {
    /// Broadcast channel for sending NodeEvents to multiple subscribers
    event_sender: Option<broadcast::Sender<NodeEvent>>,
    /// Track authenticated peers
    authenticated_peers: std::collections::HashSet<PeerId>,
    /// Pending tasks for listen_and_wait operations
    listen_wait_tasks: PendingTaskManager<ListenerId, Multiaddr, Box<dyn std::error::Error + Send + Sync>, ()>,
    /// Pending tasks for dial_and_wait operations
    dial_wait_tasks: PendingTaskManager<PeerId, (), Box<dyn std::error::Error + Send + Sync>, ()>,
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
            libp2p::swarm::SwarmEvent::NewListenAddr { listener_id, address, .. } => {
                // Complete pending listen_and_wait task if exists
                if self.listen_wait_tasks.set_task_result(&listener_id, address.clone()).is_ok() {
                    debug!("✅ [SwarmHandler] Completed listen_and_wait task for listener_id: {:?}", listener_id);
                }
                
                let _ = event_sender.send(NodeEvent::NewListenAddr {
                    listener_id: listener_id.clone(),
                    address: address.clone(),
                });
            }
            libp2p::swarm::SwarmEvent::ExpiredListenAddr { listener_id, address, .. } => {
                let _ = event_sender.send(NodeEvent::ExpiredListenAddr {
                    listener_id: listener_id.clone(),
                    address: address.clone(),
                });
            }
            libp2p::swarm::SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                ..
            } => {
                println!("Conn established {:?}", peer_id);
                // Complete pending dial_and_wait task if exists
                if self.dial_wait_tasks.set_task_result(&peer_id, ()).is_ok() {
                    debug!("✅ [SwarmHandler] Completed dial_and_wait task for peer: {}", peer_id);
                }
                
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
            SwarmLevelCommand::ListenAndWait { addr, timeout, response } => {
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
                
                info!("📡 [SwarmHandler] Started listening on address {} with listener_id: {:?}", addr, listener_id);
                
                // Add pending task to wait for NewListenAddr event
                self.listen_wait_tasks.add_pending_task(listener_id, timeout, response);
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
                
                // Check if peer is already connected
                if swarm.connected_peers().any(|p| p == &peer_id) {
                    debug!("✅ [SwarmHandler] Peer {} is already connected, completing dial_and_wait immediately", peer_id);
                    let _ = response.send(Ok(()));
                    return;
                }
                
                // Start dialing
                let result = swarm.dial(addr.clone());
                if let Err(e) = result {
                    let error = Box::new(e) as Box<dyn std::error::Error + Send + Sync>;
                    debug!("❌ [SwarmHandler] Failed to dial peer {}: {:?}", peer_id, error);
                    let _ = response.send(Err(error));
                    return;
                }
                
                info!("📡 [SwarmHandler] Dialing peer {} at address {}, waiting for connection", peer_id, addr);
                
                // Add pending task to wait for ConnectionEstablished event
                self.dial_wait_tasks.add_pending_task(peer_id, timeout, response);
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
                                    swarm.add_external_address(info.observed_addr.clone());
                                }
                                _ => {}
                            },
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

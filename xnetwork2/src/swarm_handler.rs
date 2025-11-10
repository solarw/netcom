//! Swarm handler for XNetwork2

use async_trait::async_trait;
use command_swarm::SwarmHandler;
use libp2p::{Multiaddr, PeerId, Swarm};
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::main_behaviour::{XNetworkBehaviour, XNetworkBehaviourEvent};
use crate::node_events::NodeEvent;
use crate::swarm_commands::{NetworkState, SwarmLevelCommand};
use xauth::events::PorAuthEvent;
use xstream::events::XStreamEvent;

/// Swarm handler for XNetwork2
pub struct XNetworkSwarmHandler {
    /// Broadcast channel for sending NodeEvents to multiple subscribers
    event_sender: Option<broadcast::Sender<NodeEvent>>,
    /// Track authenticated peers
    authenticated_peers: std::collections::HashSet<PeerId>,
}

impl Default for XNetworkSwarmHandler {
    fn default() -> Self {
        Self {
            event_sender: None,
            authenticated_peers: std::collections::HashSet::new(),
        }
    }
}

impl XNetworkSwarmHandler {
    /// Create a new SwarmHandler with event sender
    pub fn with_event_sender(event_sender: broadcast::Sender<NodeEvent>) -> Self {
        Self {
            event_sender: Some(event_sender),
            authenticated_peers: std::collections::HashSet::new(),
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
            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                let _ = event_sender.send(NodeEvent::NewListenAddr {
                    address: address.clone(),
                });
            }
            libp2p::swarm::SwarmEvent::ExpiredListenAddr { address, .. } => {
                let _ = event_sender.send(NodeEvent::ExpiredListenAddr {
                    address: address.clone(),
                });
            }
            libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                let _ = event_sender.send(NodeEvent::ConnectionEstablished { peer_id: *peer_id });
            }
            libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, .. } => {
                let _ = event_sender.send(NodeEvent::ConnectionClosed { peer_id: *peer_id });
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
                            PorAuthEvent::MutualAuthSuccess { peer_id, .. } => {
                                let _ = event_sender
                                    .send(NodeEvent::PeerAuthenticated { peer_id: *peer_id });
                            }
                            PorAuthEvent::OutboundAuthSuccess { peer_id, .. } => {
                                let _ = event_sender
                                    .send(NodeEvent::PeerAuthenticated { peer_id: *peer_id });
                            }
                            PorAuthEvent::InboundAuthSuccess { peer_id, .. } => {
                                let _ = event_sender
                                    .send(NodeEvent::PeerAuthenticated { peer_id: *peer_id });
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
                                println!(
                                    "🔍 [SwarmHandler] Forwarding111111111111111 IncomingStreamRequest from peer: {}, connection: {:?}",
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
                        // Forward XRoutes events to application
                        match xroutes_event {
                            _ => {
                                debug!("📡 [SwarmHandler] XRoutes event: {:?}", xroutes_event);
                            }
                        }
                    }
                    // Skip other behaviour events
                    _ => {}
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
                    .map(|_| ())
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
        }
    }

    async fn handle_event(
        &mut self,
        _swarm: &mut Swarm<XNetworkBehaviour>,
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
                    XNetworkBehaviourEvent::Identify(event) => {
                        debug!("📡 [SwarmHandler] Identify event: {:?}", event);
                    }
                    XNetworkBehaviourEvent::Ping(event) => {
                        debug!("📡 [SwarmHandler] Ping event: {:?}", event);
                    }
                    XNetworkBehaviourEvent::Xauth(event) => {
                        debug!("📡 [SwarmHandler] XAuth event: {:?}", event);
                        println!("📡 [SwarmHandler] XAuth event: {:?}", event);

                        // Добавляем специальную отладочную информацию для событий аутентификации
                        match event {
                            PorAuthEvent::MutualAuthSuccess { peer_id, .. } => {
                                println!(
                                    "🎉 [SwarmHandler] MUTUAL AUTH SUCCESS for peer: {}",
                                    peer_id
                                );
                            }
                            PorAuthEvent::OutboundAuthSuccess { peer_id, .. } => {
                                println!(
                                    "✅ [SwarmHandler] OUTBOUND AUTH SUCCESS for peer: {}",
                                    peer_id
                                );
                            }
                            PorAuthEvent::InboundAuthSuccess { peer_id, .. } => {
                                println!(
                                    "✅ [SwarmHandler] INBOUND AUTH SUCCESS for peer: {}",
                                    peer_id
                                );
                            }
                            PorAuthEvent::OutboundAuthFailure { peer_id, .. } => {
                                println!(
                                    "❌ [SwarmHandler] OUTBOUND AUTH FAILURE for peer: {}",
                                    peer_id
                                );
                            }
                            PorAuthEvent::InboundAuthFailure { peer_id, .. } => {
                                println!(
                                    "❌ [SwarmHandler] INBOUND AUTH FAILURE for peer: {}",
                                    peer_id
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
                    }
                }
            }
            _ => {
                debug!("🌐 [SwarmHandler] Swarm event: {:?}", event);
                println!("🌐 [SwarmHandler] Swarm event: {:?}", event);
            }
        }
    }
}

use libp2p::{
    relay,
    swarm::{
        NetworkBehaviour, ToSwarm, derive_prelude::*,
    },
    Multiaddr, PeerId,
    core::multiaddr::Protocol,
};
use std::collections::{HashMap, VecDeque};
use std::task::{Context, Poll};
use libp2p::core::transport::ListenerId;

use super::commands::RelayClientCommand;
use super::events::{RelayClientEvent, RelayClientStats};

pub struct RelayClientBehaviour {
    relay_client: relay::client::Behaviour,
    local_peer_id: PeerId,
    stats: RelayClientStats,
    pending_commands: VecDeque<RelayClientCommand>,
    pending_events: VecDeque<RelayClientEvent>,
    active_reservations: HashMap<PeerId, Multiaddr>,
    listening_relays: HashMap<Multiaddr, (PeerId, Option<ListenerId>)>, // relay_addr -> (relay_peer_id, listener_id)
    pending_reservations: HashMap<PeerId, Multiaddr>, // relay_peer_id -> relay_addr
}

#[derive(Debug)]
pub enum RelayClientBehaviourEvent {
    RelayClient(relay::client::Event),
    RelayClientEvent(RelayClientEvent),
}

impl RelayClientBehaviour {
    pub fn new(local_peer_id: PeerId) -> Self {
        // Create relay client using the transport method but don't store transport
        // The transport is managed separately at the swarm level
        let (_transport, relay_client) = relay::client::new(local_peer_id);
        // Note: We drop the transport here as it's handled by the swarm
        
        Self {
            relay_client,
            local_peer_id,
            stats: RelayClientStats::default(),
            pending_commands: VecDeque::new(),
            pending_events: VecDeque::new(),
            active_reservations: HashMap::new(),
            listening_relays: HashMap::new(),
            pending_reservations: HashMap::new(),
        }
    }

    pub fn handle_command(&mut self, command: RelayClientCommand) {
        self.pending_commands.push_back(command);
    }

    pub fn stats(&self) -> &RelayClientStats {
        &self.stats
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn active_reservations(&self) -> &HashMap<PeerId, Multiaddr> {
        &self.active_reservations
    }

    fn process_pending_commands(&mut self) -> Option<ToSwarm<RelayClientBehaviourEvent, THandlerInEvent<Self>>> {
        if let Some(command) = self.pending_commands.pop_front() {
            match command {
                RelayClientCommand::ListenViaRelay { relay_addr, response } => {
                    if let Some(relay_peer_id) = extract_peer_id_from_addr(&relay_addr) {
                        // Store the listening relay (without ListenerId initially)
                        self.listening_relays.insert(relay_addr.clone(), (relay_peer_id, None));
                        
                        // Create circuit relay listen address
                        let listen_addr = relay_addr.clone()
                            .with(Protocol::P2pCircuit);
                        
                        let _ = response.send(Ok(listen_addr.clone()));
                        
                        self.pending_events.push_back(RelayClientEvent::ListeningViaRelay {
                            relay_addr: relay_addr.clone(),
                            listen_addr: listen_addr.clone(),
                        });
                        
                        // Check if we need to make a reservation first
                        if !self.active_reservations.contains_key(&relay_peer_id) {
                            // Store pending reservation
                            self.pending_reservations.insert(relay_peer_id, relay_addr.clone());
                            self.stats.total_reservations_made += 1;
                            
                            // Dial to relay server to make reservation
                            return Some(ToSwarm::Dial {
                                opts: relay_addr.into(),
                            });
                        } else {
                            // We already have a reservation, start listening immediately
                            return Some(ToSwarm::ListenOn {
                                opts: listen_addr.into(),
                            });
                        }
                    } else {
                        let _ = response.send(Err("Invalid relay address format".to_string()));
                    }
                }

                RelayClientCommand::StopListeningViaRelay { relay_addr, response } => {
                    if let Some((relay_peer_id, listener_id)) = self.listening_relays.remove(&relay_addr) {
                        let _ = response.send(Ok(()));
                        self.pending_events.push_back(RelayClientEvent::StoppedListeningViaRelay {
                            relay_addr: relay_addr.clone(),
                        });
                        
                        // If we have a ListenerId, properly remove the listener
                        if let Some(listener_id) = listener_id {
                            return Some(ToSwarm::RemoveListener {
                                id: listener_id,
                            });
                        }
                    } else {
                        let _ = response.send(Err("Not listening via this relay".to_string()));
                    }
                }

                RelayClientCommand::GetStats { response } => {
                    self.stats.active_reservations = self.active_reservations.len();
                    let _ = response.send(self.stats.clone());
                }
            }
        }
        None
    }

    fn handle_relay_client_event(&mut self, event: &relay::client::Event) {
        match event {
            relay::client::Event::ReservationReqAccepted {
                relay_peer_id,
                renewal: _,
                limit: _,
            } => {
                // Check if this was a pending reservation
                if let Some(relay_addr) = self.pending_reservations.remove(relay_peer_id) {
                    // Store the active reservation
                    self.active_reservations.insert(*relay_peer_id, relay_addr.clone());
                    self.stats.active_reservations = self.active_reservations.len();
                    
                    self.pending_events.push_back(RelayClientEvent::ReservationMade {
                        relay_peer_id: *relay_peer_id,
                        relay_addr: relay_addr.clone(),
                    });
                    
                    // If we were trying to listen on this relay, start listening now
                    if self.listening_relays.values().any(|(id, _)| *id == *relay_peer_id) {
                        let listen_addr = relay_addr.with(Protocol::P2pCircuit);
                        // We'll handle the actual listening in the next poll cycle
                        // by returning a ListenOn command
                    }
                }
            }

            relay::client::Event::OutboundCircuitEstablished {
                relay_peer_id: _,
                limit: _,
            } => {
                self.stats.successful_outbound_connections += 1;
            }

            relay::client::Event::InboundCircuitEstablished {
                src_peer_id,
                limit: _,
            } => {
                self.stats.incoming_connections_via_relay += 1;
                
                // Find the relay peer ID from listening relays
                for (_, (relay_peer_id, _)) in &self.listening_relays {
                    self.pending_events.push_back(RelayClientEvent::IncomingConnectionViaRelay {
                        relay_peer_id: *relay_peer_id,
                        remote_peer_id: *src_peer_id,
                    });
                    break;
                }
            }

            // Handle reservation failures
            _ => {
                // For other events, we can add specific handling as needed
                // For now, we'll handle them minimally
            }
        }
    }
}

impl NetworkBehaviour for RelayClientBehaviour {
    type ConnectionHandler = <relay::client::Behaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = RelayClientBehaviourEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.relay_client.handle_established_inbound_connection(connection_id, peer, local_addr, remote_addr)
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.relay_client.handle_established_outbound_connection(connection_id, peer, addr, role_override, port_use)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        self.relay_client.on_swarm_event(event);
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.relay_client.on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Return pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(RelayClientBehaviourEvent::RelayClientEvent(event)));
        }

        // Process pending commands
        if let Some(to_swarm) = self.process_pending_commands() {
            return Poll::Ready(to_swarm);
        }

        // Poll the underlying relay client behaviour
        loop {
            match self.relay_client.poll(cx) {
                Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                    self.handle_relay_client_event(&event);
                    return Poll::Ready(ToSwarm::GenerateEvent(RelayClientBehaviourEvent::RelayClient(event)));
                }
                Poll::Ready(other_event) => {
                    return Poll::Ready(other_event.map_out(|_| unreachable!()).map_in(|e| e));
                }
                Poll::Pending => break,
            }
        }

        // Check for pending events one more time
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(RelayClientBehaviourEvent::RelayClientEvent(event)));
        }

        Poll::Pending
    }
}

// Helper function to extract peer ID from multiaddr
fn extract_peer_id_from_addr(addr: &Multiaddr) -> Option<PeerId> {
    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}

impl From<relay::client::Event> for RelayClientBehaviourEvent {
    fn from(event: relay::client::Event) -> Self {
        RelayClientBehaviourEvent::RelayClient(event)
    }
}

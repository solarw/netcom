use libp2p::{
    relay,
    swarm::{
        NetworkBehaviour, ToSwarm, derive_prelude::*,
    },
    Multiaddr, PeerId,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use super::commands::RelayServerCommand;
use super::events::{RelayServerEvent, RelayServerStats};

pub struct RelayServerBehaviour {
    relay_server: Option<relay::Behaviour>,
    local_peer_id: PeerId,
    is_running: bool,
    listen_addr: Option<Multiaddr>,
    stats: RelayServerStats,
    pending_commands: VecDeque<RelayServerCommand>,
    pending_events: VecDeque<RelayServerEvent>,
    active_reservations: HashSet<PeerId>,
    active_circuits: HashMap<(PeerId, PeerId), Instant>,
    config: RelayServerConfig,
    start_time: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct RelayServerConfig {
    pub max_reservations: usize,
    pub max_circuits: usize,
    pub reservation_duration: Duration,
    pub circuit_duration: Duration,
}

impl Default for RelayServerConfig {
    fn default() -> Self {
        Self {
            max_reservations: 128,
            max_circuits: 16,
            reservation_duration: Duration::from_secs(3600), // 1 hour
            circuit_duration: Duration::from_secs(120),      // 2 minutes
        }
    }
}

#[derive(Debug)]
pub enum RelayServerBehaviourEvent {
    RelayServerEvent(RelayServerEvent),
}

impl RelayServerBehaviour {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            relay_server: None,
            local_peer_id,
            is_running: false,
            listen_addr: None,
            stats: RelayServerStats::default(),
            pending_commands: VecDeque::new(),
            pending_events: VecDeque::new(),
            active_reservations: HashSet::new(),
            active_circuits: HashMap::new(),
            config: RelayServerConfig::default(),
            start_time: None,
        }
    }

    pub fn handle_command(&mut self, command: RelayServerCommand) {
        self.pending_commands.push_back(command);
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn stats(&self) -> &RelayServerStats {
        &self.stats
    }

    pub fn config(&self) -> &RelayServerConfig {
        &self.config
    }

    fn process_pending_commands(&mut self) -> Option<ToSwarm<RelayServerBehaviourEvent, THandlerInEvent<Self>>> {
        if let Some(command) = self.pending_commands.pop_front() {
            match command {
                RelayServerCommand::Start { listen_addr, response } => {
                    if self.is_running {
                        let _ = response.send(Err("Relay server is already running".to_string()));
                        return None;
                    }

                    // Create relay server behaviour with configuration like in the example
                    let relay_config = relay::Config {
                        max_reservations: self.config.max_reservations,
                        max_circuits: self.config.max_circuits,
                        reservation_duration: self.config.reservation_duration,
                        max_circuit_duration: self.config.circuit_duration,
                        ..Default::default()
                    };

                    // Use the actual local peer ID like in the example
                    self.relay_server = Some(relay::Behaviour::new(self.local_peer_id, relay_config));
                    self.is_running = true;
                    self.start_time = Some(Instant::now());

                    let addr = listen_addr.unwrap_or_else(|| "/ip4/0.0.0.0/tcp/0".parse().unwrap());
                    self.listen_addr = Some(addr.clone());

                    let _ = response.send(Ok(addr.clone()));

                    self.pending_events.push_back(RelayServerEvent::ServerStarted {
                        listen_addr: addr.clone(),
                    });

                    // Generate listen event
                    return Some(ToSwarm::ListenOn {
                        opts: addr.into(),
                    });
                }

                RelayServerCommand::Stop { response } => {
                    if !self.is_running {
                        let _ = response.send(Err("Relay server is not running".to_string()));
                        return None;
                    }

                    self.relay_server = None;
                    self.is_running = false;
                    self.listen_addr = None;
                    self.active_reservations.clear();
                    self.active_circuits.clear();
                    self.start_time = None;

                    // Reset stats
                    self.stats.active_reservations = 0;
                    self.stats.active_circuits = 0;

                    let _ = response.send(Ok(()));

                    self.pending_events.push_back(RelayServerEvent::ServerStopped);
                }

                RelayServerCommand::GetStatus { response } => {
                    let _ = response.send(self.is_running);
                }

                RelayServerCommand::GetStats { response } => {
                    // Update uptime
                    if let Some(start_time) = self.start_time {
                        self.stats.uptime_seconds = start_time.elapsed().as_secs();
                    }
                    let _ = response.send(self.stats.clone());
                }

                RelayServerCommand::Configure {
                    max_reservations,
                    max_circuits,
                    response,
                } => {
                    if let Some(max_res) = max_reservations {
                        self.config.max_reservations = max_res;
                    }
                    if let Some(max_circ) = max_circuits {
                        self.config.max_circuits = max_circ;
                    }

                    let _ = response.send(Ok(()));

                    self.pending_events.push_back(RelayServerEvent::ConfigurationChanged {
                        max_reservations: self.config.max_reservations,
                        max_circuits: self.config.max_circuits,
                    });
                }

                RelayServerCommand::GetConfiguration { response } => {
                    let _ = response.send((self.config.max_reservations, self.config.max_circuits));
                }

                RelayServerCommand::CloseCircuit {
                    src_peer_id,
                    dst_peer_id,
                    response,
                } => {
                    let circuit_key = (src_peer_id, dst_peer_id);
                    if self.active_circuits.remove(&circuit_key).is_some() {
                        self.stats.active_circuits = self.stats.active_circuits.saturating_sub(1);
                        let _ = response.send(Ok(()));

                        self.pending_events.push_back(RelayServerEvent::CircuitClosed {
                            src_peer_id,
                            dst_peer_id,
                        });
                    } else {
                        let _ = response.send(Err("Circuit not found".to_string()));
                    }
                }

                RelayServerCommand::ListReservations { response } => {
                    let reservations: Vec<PeerId> = self.active_reservations.iter().copied().collect();
                    let _ = response.send(reservations);
                }

                RelayServerCommand::ListCircuits { response } => {
                    let circuits: Vec<(PeerId, PeerId)> = self.active_circuits.keys().copied().collect();
                    let _ = response.send(circuits);
                }
            }
        }
        None
    }

    fn handle_relay_event(&mut self, event: &relay::Event) {
        match event {
            relay::Event::ReservationReqAccepted {
                src_peer_id,
                renewed,
            } => {
                if !renewed {
                    self.active_reservations.insert(*src_peer_id);
                    self.stats.active_reservations = self.active_reservations.len();
                    self.stats.total_reservations_handled += 1;

                    self.pending_events.push_back(RelayServerEvent::ReservationMade {
                        client_peer_id: *src_peer_id,
                    });
                }
            }

            relay::Event::ReservationTimedOut { src_peer_id } => {
                self.active_reservations.remove(src_peer_id);
                self.stats.active_reservations = self.active_reservations.len();

                self.pending_events.push_back(RelayServerEvent::ReservationExpired {
                    client_peer_id: *src_peer_id,
                });
            }

            relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
            } => {
                let circuit_key = (*src_peer_id, *dst_peer_id);
                self.active_circuits.insert(circuit_key, Instant::now());
                self.stats.active_circuits = self.active_circuits.len();
                self.stats.total_circuits_handled += 1;

                self.pending_events.push_back(RelayServerEvent::CircuitEstablished {
                    src_peer_id: *src_peer_id,
                    dst_peer_id: *dst_peer_id,
                });
            }

            relay::Event::CircuitReqDenied {
                src_peer_id,
                dst_peer_id,
            } => {
                self.stats.failed_circuit_requests += 1;

                self.pending_events.push_back(RelayServerEvent::CircuitFailed {
                    src_peer_id: *src_peer_id,
                    dst_peer_id: *dst_peer_id,
                    error: "Circuit request denied".to_string(),
                });
            }

            relay::Event::CircuitClosed {
                src_peer_id,
                dst_peer_id,
                error: _,
            } => {
                let circuit_key = (*src_peer_id, *dst_peer_id);
                self.active_circuits.remove(&circuit_key);
                self.stats.active_circuits = self.active_circuits.len();

                self.pending_events.push_back(RelayServerEvent::CircuitClosed {
                    src_peer_id: *src_peer_id,
                    dst_peer_id: *dst_peer_id,
                });
            }

            relay::Event::ReservationClosed { src_peer_id } => {
                self.active_reservations.remove(src_peer_id);
                self.stats.active_reservations = self.active_reservations.len();

                self.pending_events.push_back(RelayServerEvent::ReservationExpired {
                    client_peer_id: *src_peer_id,
                });
            }

            relay::Event::CircuitReqOutboundConnectFailed { src_peer_id, dst_peer_id, error } => {
                self.stats.failed_circuit_requests += 1;

                self.pending_events.push_back(RelayServerEvent::CircuitFailed {
                    src_peer_id: *src_peer_id,
                    dst_peer_id: *dst_peer_id,
                    error: format!("{:?}", error),
                });
            }

            relay::Event::CircuitReqAcceptFailed { src_peer_id, dst_peer_id, error } => {
                self.stats.failed_circuit_requests += 1;

                self.pending_events.push_back(RelayServerEvent::CircuitFailed {
                    src_peer_id: *src_peer_id,
                    dst_peer_id: *dst_peer_id,
                    error: format!("{:?}", error),
                });
            }

            // Handle other events silently - we only emit high-level events
            _ => {}
        }
    }

    fn cleanup_expired_circuits(&mut self) {
        let now = Instant::now();
        let expired_circuits: Vec<(PeerId, PeerId)> = self
            .active_circuits
            .iter()
            .filter(|(_, start_time)| now.duration_since(**start_time) > self.config.circuit_duration)
            .map(|(key, _)| *key)
            .collect();

        for circuit_key in expired_circuits {
            self.active_circuits.remove(&circuit_key);
            self.stats.active_circuits = self.active_circuits.len();

            self.pending_events.push_back(RelayServerEvent::CircuitClosed {
                src_peer_id: circuit_key.0,
                dst_peer_id: circuit_key.1,
            });
        }
    }
}

impl NetworkBehaviour for RelayServerBehaviour {
    type ConnectionHandler = <relay::Behaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = RelayServerBehaviourEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if let Some(ref mut relay) = self.relay_server {
            relay.handle_established_inbound_connection(connection_id, peer, local_addr, remote_addr)
        } else {
            Err(ConnectionDenied::new("Relay server not running"))
        }
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if let Some(ref mut relay) = self.relay_server {
            relay.handle_established_outbound_connection(connection_id, peer, addr, role_override, port_use)
        } else {
            Err(ConnectionDenied::new("Relay server not running"))
        }
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        if let Some(ref mut relay) = self.relay_server {
            relay.on_swarm_event(event);
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        if let Some(ref mut relay) = self.relay_server {
            relay.on_connection_handler_event(peer_id, connection_id, event);
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Cleanup expired circuits
        self.cleanup_expired_circuits();

        // Return pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(RelayServerBehaviourEvent::RelayServerEvent(event)));
        }

        // Process pending commands
        if let Some(to_swarm) = self.process_pending_commands() {
            return Poll::Ready(to_swarm);
        }

        // Poll the underlying relay behaviour if running
        let mut relay_events = Vec::new();
        if let Some(ref mut relay) = self.relay_server {
            loop {
                match relay.poll(cx) {
                    Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                        // Collect events to handle after polling
                        relay_events.push(event);
                    }
                    Poll::Ready(other_event) => {
                        return Poll::Ready(other_event.map_out(|_| unreachable!()).map_in(|e| e));
                    }
                    Poll::Pending => break,
                }
            }
        }
        
        // Handle collected relay events (after relay borrow is dropped)
        for event in relay_events {
            self.handle_relay_event(&event);
        }

        // Check for pending events one more time
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(RelayServerBehaviourEvent::RelayServerEvent(event)));
        }

        Poll::Pending
    }
}

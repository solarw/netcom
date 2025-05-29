use libp2p::{
    swarm::{
        NetworkBehaviour, ToSwarm, derive_prelude::*,
    },
    PeerId,
};
use std::collections::VecDeque;
use std::task::{Context, Poll};

use super::commands::ConnectivityCommand;
use super::events::ConnectivityEvent;
use super::relay_server::{RelayServerBehaviour, RelayServerBehaviourEvent, RelayServerCommand};

pub struct ConnectivityBehaviour {
    relay_server: RelayServerBehaviour,
    pending_events: VecDeque<ConnectivityEvent>,
}

#[derive(Debug)]
pub enum ConnectivityBehaviourEvent {
    RelayServer(RelayServerBehaviourEvent),
    ConnectivityEvent(ConnectivityEvent),
}

impl ConnectivityBehaviour {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            relay_server: RelayServerBehaviour::new(local_peer_id),
            pending_events: VecDeque::new(),
        }
    }

    pub fn new_with_config(local_peer_id: PeerId, config: &crate::xroutes::XRoutesConfig) -> Self {
        let behaviour = Self::new(local_peer_id);
        
        // Note: Known relay servers are now handled internally by the relay client
        // when making reservations during listen operations
        
        behaviour
    }

    pub fn handle_command(&mut self, command: ConnectivityCommand) {
        match command {
            ConnectivityCommand::RelayServer(relay_server_command) => {
                self.relay_server.handle_command(relay_server_command);
            }
        }
    }


    pub fn relay_server(&self) -> &RelayServerBehaviour {
        &self.relay_server
    }

    pub fn relay_server_mut(&mut self) -> &mut RelayServerBehaviour {
        &mut self.relay_server
    }

    /// Poll for pending events
    pub fn poll_pending_events(&mut self) -> Option<ConnectivityEvent> {
        self.pending_events.pop_front()
    }

    /// Add an event to the pending events queue
    pub fn push_event(&mut self, event: ConnectivityEvent) {
        self.pending_events.push_back(event);
    }
}

impl NetworkBehaviour for ConnectivityBehaviour {
    type ConnectionHandler =         <RelayServerBehaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = ConnectivityBehaviourEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &libp2p::Multiaddr,
        remote_addr: &libp2p::Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Only relay server is available now
        self.relay_server.handle_established_inbound_connection(connection_id, peer, local_addr, remote_addr)
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &libp2p::Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Only relay server is available now
        self.relay_server.handle_established_outbound_connection(connection_id, peer, addr, role_override, port_use)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        self.relay_server.on_swarm_event(event);
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.relay_server.on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Return pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(ConnectivityBehaviourEvent::ConnectivityEvent(event)));
        }

        // Poll the relay server behaviour
        loop {
            match self.relay_server.poll(cx) {
                Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                    return Poll::Ready(ToSwarm::GenerateEvent(ConnectivityBehaviourEvent::RelayServer(event)));
                }
                Poll::Ready(other_event) => {
                    return Poll::Ready(other_event.map_out(|_| unreachable!()));
                }
                Poll::Pending => break,
            }
        }

        // Check for pending events one more time
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(ConnectivityBehaviourEvent::ConnectivityEvent(event)));
        }

        Poll::Pending
    }
}



impl From<RelayServerBehaviourEvent> for ConnectivityBehaviourEvent {
    fn from(event: RelayServerBehaviourEvent) -> Self {
        ConnectivityBehaviourEvent::RelayServer(event)
    }
}

impl From<ConnectivityEvent> for ConnectivityBehaviourEvent {
    fn from(event: ConnectivityEvent) -> Self {
        ConnectivityBehaviourEvent::ConnectivityEvent(event)
    }
}

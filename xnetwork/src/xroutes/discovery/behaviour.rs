// ./xroutes/discovery/behaviour.rs

use libp2p::{
    Multiaddr, PeerId, identity,
    swarm::{
        NetworkBehaviour, THandlerInEvent, ToSwarm, derive_prelude::*,
    },
};
use std::collections::VecDeque;
use std::task::{Context, Poll};
use tracing::{info, debug};

use super::commands::DiscoveryCommand;
use super::events::DiscoveryEvent;
use super::mdns::behaviour::MdnsBehaviour;
use super::kad::behaviour::KadBehaviour;

pub struct DiscoveryBehaviour {
    pub mdns: MdnsBehaviour,
    pub kad: KadBehaviour,
    pending_events: VecDeque<DiscoveryEvent>,
}

impl DiscoveryBehaviour {
    pub fn new(
        key: &identity::Keypair,
        enable_mdns: bool,
        enable_kad: bool,
        kad_server_mode: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(DiscoveryBehaviour {
            mdns: MdnsBehaviour::new(key, enable_mdns)?,
            kad: KadBehaviour::new(key, enable_kad, kad_server_mode)?,
            pending_events: VecDeque::new(),
        })
    }

    pub fn handle_command(&mut self, cmd: DiscoveryCommand) {
        match cmd {
            DiscoveryCommand::Mdns(mdns_command) => {
                self.mdns.handle_command(mdns_command);
            }
            DiscoveryCommand::Kad(kad_command) => {
                self.kad.handle_command(kad_command);
            }
        }
    }
}

impl NetworkBehaviour for DiscoveryBehaviour {
    type ConnectionHandler = libp2p::swarm::dummy::ConnectionHandler;
    type ToSwarm = DiscoveryEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(libp2p::swarm::dummy::ConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
        _port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(libp2p::swarm::dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Forward relevant events to child behaviours
        self.mdns.on_swarm_event(event.clone());
        self.kad.on_swarm_event(event.clone());
        
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
        // Dummy handler produces no events
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Return pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        // Poll mDNS for events
        loop {
            match self.mdns.poll(cx) {
                Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                    return Poll::Ready(ToSwarm::GenerateEvent(event));
                }
                Poll::Ready(_other_event) => {
                    // Ignore non-event ToSwarm variants from mDNS
                    continue;
                }
                Poll::Pending => break,
            }
        }

        // Poll Kademlia for events
        loop {
            match self.kad.poll(cx) {
                Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                    return Poll::Ready(ToSwarm::GenerateEvent(event));
                }
                Poll::Ready(_other_event) => {
                    // Ignore non-event ToSwarm variants from Kademlia for now
                    // In a more sophisticated implementation, we might want to
                    // forward these events appropriately
                    continue;
                }
                Poll::Pending => break,
            }
        }

        // Check for pending events one more time
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        Poll::Pending
    }
}
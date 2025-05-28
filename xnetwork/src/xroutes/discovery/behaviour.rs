// XRoutes discovery behaviour combining mDNS and Kademlia
use libp2p::Multiaddr;
use libp2p::PeerId;
use libp2p::core::Endpoint;
use libp2p::core::transport::PortUse;
use libp2p::identity;
use libp2p::swarm::ConnectionDenied;
use libp2p::swarm::ConnectionId;
use libp2p::swarm::FromSwarm;
use libp2p::swarm::NetworkBehaviour;
use libp2p::swarm::THandler;
use libp2p::swarm::THandlerInEvent;
use libp2p::swarm::THandlerOutEvent;
use libp2p::swarm::ToSwarm;
use libp2p::swarm::behaviour::toggle::Toggle;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::task::{Context, Poll};
use tracing::info;

use super::commands::DiscoveryCommand;
use super::events::DiscoveryEvent;
use super::mdns;
use super::mdns::behaviour::MdnsBehaviour;

pub struct DiscoveryBehaviour {
    pub mdns: MdnsBehaviour,
}

impl DiscoveryBehaviour {
    pub fn new(
        key: &identity::Keypair,
        enable_mdns: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(DiscoveryBehaviour {
            mdns: MdnsBehaviour::new(key, enable_mdns)?,
        })
    }
    fn handle_command(&mut self, cmd: DiscoveryCommand) {
        match cmd {
            DiscoveryCommand::Mdns(mdns_command) => self.mdns.handle_command(mdns_command),
        }
    }
}

impl NetworkBehaviour for DiscoveryBehaviour {
    type ConnectionHandler = <MdnsBehaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = DiscoveryEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.mdns.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.mdns.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        self.mdns.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.mdns
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Получаем события от mdns
        match self.mdns.poll(cx) {
            Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                return Poll::Ready(ToSwarm::GenerateEvent(event));
            }
            Poll::Ready(some_event) => {
                info!("Discovery some event {:?}", some_event);
            }

            Poll::Pending => {}
        }
        Poll::Pending
    }
}

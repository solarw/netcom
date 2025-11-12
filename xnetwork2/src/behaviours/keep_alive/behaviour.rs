//! Custom NetworkBehaviour for KeepAlive

use std::task::{Context, Poll};
use libp2p::swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, NotifyHandler, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};

use super::handler::KeepAliveConnectionHandler;

/// Custom NetworkBehaviour for KeepAlive
#[derive(Default)]
pub struct KeepAliveBehaviour {
    /// Keep-alive status
    enabled: bool,
}

/// Events emitted by KeepAliveBehaviour
#[derive(Debug)]
pub enum KeepAliveEvent {
    /// No events for now, but required by the trait
    Dummy,
}

impl KeepAliveBehaviour {
    /// Create a new KeepAliveBehaviour
    pub fn new() -> Self {
        Self {
            enabled: true, // Default to enabled
        }
    }

    /// Set keep-alive status
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get keep-alive status
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl NetworkBehaviour for KeepAliveBehaviour {
    type ConnectionHandler = KeepAliveConnectionHandler;
    type ToSwarm = KeepAliveEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<KeepAliveConnectionHandler, ConnectionDenied> {
        Ok(KeepAliveConnectionHandler::new(self.enabled))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
        _port_use: libp2p::core::transport::PortUse,
    ) -> Result<KeepAliveConnectionHandler, ConnectionDenied> {
        Ok(KeepAliveConnectionHandler::new(self.enabled))
    }

    fn on_swarm_event(&mut self, _event: FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        _event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        Poll::Pending
    }
}

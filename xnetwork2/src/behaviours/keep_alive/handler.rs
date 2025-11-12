//! ConnectionHandler for KeepAliveBehaviour

use std::task::{Context, Poll};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, ConnectionId, SubstreamProtocol,
};
use libp2p::core::upgrade::DeniedUpgrade;

/// ConnectionHandler for KeepAliveBehaviour
pub struct KeepAliveConnectionHandler {
    /// Keep-alive status
    enabled: bool,
}

impl KeepAliveConnectionHandler {
    /// Create a new KeepAliveConnectionHandler
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl ConnectionHandler for KeepAliveConnectionHandler {
    type FromBehaviour = ();
    type ToBehaviour = ();
    type InboundProtocol = DeniedUpgrade;
    type OutboundProtocol = DeniedUpgrade;
    type OutboundOpenInfo = ();
    type InboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(DeniedUpgrade, ())
    }

    fn connection_keep_alive(&self) -> bool {
        self.enabled
    }

    fn on_behaviour_event(&mut self, _: Self::FromBehaviour) {}

    fn on_connection_event(
        &mut self,
        _: libp2p::swarm::handler::ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
    }

    fn poll(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>> {
        Poll::Pending
    }
}

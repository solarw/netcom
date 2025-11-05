//! EchoBehaviour for demonstrating the system operation

use libp2p::PeerId;
use libp2p::swarm::{ConnectionDenied, NetworkBehaviour, ToSwarm};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::task::{Context, Poll};

use super::event::EchoEvent;

/// Simple behaviour for demonstrating the system operation
#[derive(Default)]
pub struct EchoBehaviour {
    /// Internal state
    messages: Vec<String>,
    /// Event queue for generation in Swarm
    pending_events: VecDeque<EchoEvent>,
}

impl EchoBehaviour {
    /// Create new EchoBehaviour
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            pending_events: VecDeque::new(),
        }
    }

    /// Send message (method that can be called via command)
    pub fn send_message(&mut self, peer_id: PeerId, text: String) {
        println!(
            "📤 [EchoBehaviour] Sending message - Peer: {:?}, Text: {}",
            peer_id, text
        );

        // Save message for demonstration
        self.messages.push(text.clone());

        // Add event to queue for generation in Swarm
        let event = EchoEvent::MessageReceived { peer_id, text };
        self.pending_events.push_back(event);
        println!("📤 [EchoBehaviour] Event added to Swarm queue");
    }

    /// Get count of sent messages
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl NetworkBehaviour for EchoBehaviour {
    type ConnectionHandler = libp2p::swarm::dummy::ConnectionHandler;
    type ToSwarm = EchoEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _: libp2p::swarm::ConnectionId,
        _: PeerId,
        _: &libp2p::Multiaddr,
        _: &libp2p::Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, ConnectionDenied> {
        Ok(libp2p::swarm::dummy::ConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: libp2p::swarm::ConnectionId,
        _: PeerId,
        _: &libp2p::Multiaddr,
        _: libp2p::core::Endpoint,
        _: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, ConnectionDenied> {
        Ok(libp2p::swarm::dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, _: libp2p::swarm::FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        _: PeerId,
        _: libp2p::swarm::ConnectionId,
        _: Infallible,
    ) {
    }

    fn poll(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        // Check if there are events in queue for generation
        if let Some(event) = self.pending_events.pop_front() {
            println!(
                "📤 EchoBehaviour::poll: Generating event for Swarm: {:?}",
                event
            );
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        Poll::Pending
    }
}

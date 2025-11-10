//! Handler for XStream behaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use tracing::{debug, info};
use xstream::behaviour::XStreamNetworkBehaviour;

use super::command::XStreamCommand;

/// Handler for XStream behaviour
#[derive(Default)]
pub struct XStreamHandler;

#[async_trait]
impl BehaviourHandler for XStreamHandler {
    type Behaviour = XStreamNetworkBehaviour;
    type Event = xstream::events::XStreamEvent;
    type Command = XStreamCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            XStreamCommand::OpenStream { peer_id, response } => {
                debug!(
                    "🔄 [XStreamHandler] Processing OpenStream command - Peer: {:?}",
                    peer_id
                );

                // Открываем XStream к указанному пиру
                behaviour.open_stream(peer_id, response).await;
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            xstream::events::XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                info!(
                    " [XStreamHandler] Stream established - Peer: {:?}, Stream ID: {:?}",
                    peer_id, stream_id
                );
            }
            xstream::events::XStreamEvent::StreamClosed { peer_id, stream_id } => {
                debug!(
                    "📤 [XStreamHandler] Stream closed - Peer: {:?}, Stream ID: {:?}",
                    peer_id, stream_id
                );
            }
            xstream::events::XStreamEvent::StreamError {
                peer_id,
                stream_id,
                error,
            } => {
                debug!(
                    "❌ [XStreamHandler] Stream error - Peer: {:?}, Stream ID: {:?}, Error: {}",
                    peer_id, stream_id, error
                );
            }
            xstream::events::XStreamEvent::IncomingStream { stream } => {
                debug!(
                    " [XStreamHandler] Incoming stream - Peer: {:?}, Stream ID: {:?}",
                    stream.peer_id, stream.id
                );
            }
            xstream::events::XStreamEvent::IncomingStreamRequest { .. } => {
                debug!("📥 [XStreamHandler] Incoming stream request received");
                // This event is handled by the swarm handler for decision making
            }
        }
    }
}

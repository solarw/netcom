//! Handler for XStream behaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use xstream::behaviour::XStreamNetworkBehaviour;
use tracing::{debug, info};

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
            XStreamCommand::OpenStream { peer_id } => {
                debug!("🔄 [XStreamHandler] Processing OpenStream command for peer: {:?}", peer_id);
                // Note: XStream automatically handles stream opening on authenticated connections
                info!("📡 [XStreamHandler] Stream will be opened automatically for authenticated peer: {:?}", peer_id);
            }
            XStreamCommand::SendData { peer_id, data } => {
                debug!("🔄 [XStreamHandler] Processing SendData command for peer: {:?}, data size: {}", peer_id, data.len());
                // Note: XStream handles data sending through established streams
                info!("📤 [XStreamHandler] Data will be sent through established stream to peer: {:?}", peer_id);
            }
            XStreamCommand::CloseStream { peer_id } => {
                debug!("🔄 [XStreamHandler] Processing CloseStream command for peer: {:?}", peer_id);
                // Note: XStream automatically manages stream lifecycle
                info!("📤 [XStreamHandler] Stream will be closed for peer: {:?}", peer_id);
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            xstream::events::XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                info!(
                    "📡 [XStreamHandler] Stream established - Peer: {:?}, Stream ID: {:?}",
                    peer_id, stream_id
                );
            }
            xstream::events::XStreamEvent::StreamClosed { peer_id, stream_id } => {
                debug!(
                    "📤 [XStreamHandler] Stream closed - Peer: {:?}, Stream ID: {:?}",
                    peer_id, stream_id
                );
            }
            xstream::events::XStreamEvent::StreamError { peer_id, stream_id, error } => {
                debug!(
                    "❌ [XStreamHandler] Stream error - Peer: {:?}, Stream ID: {:?}, Error: {}",
                    peer_id, stream_id, error
                );
            }
            xstream::events::XStreamEvent::IncomingStream { stream } => {
                debug!(
                    "📨 [XStreamHandler] Incoming stream - Peer: {:?}, Stream ID: {:?}",
                    stream.peer_id, stream.id
                );
            }
        }
    }
}

//! Handler for IdentifyBehaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use libp2p::identify;
use tracing::{debug, info};

use super::command::IdentifyCommand;

/// Handler for IdentifyBehaviour
#[derive(Default)]
pub struct IdentifyHandler;

#[async_trait]
impl BehaviourHandler for IdentifyHandler {
    type Behaviour = identify::Behaviour;
    type Event = identify::Event;
    type Command = IdentifyCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            IdentifyCommand::RequestIdentify { peer_id } => {
                debug!("🔄 [IdentifyHandler] Processing RequestIdentify command for peer: {:?}", peer_id);
                // Note: Identify protocol doesn't have explicit request method in libp2p
                // The protocol automatically exchanges identify information on connection
                info!("📡 [IdentifyHandler] Identify information will be exchanged automatically on connection with peer: {:?}", peer_id);
            }
            IdentifyCommand::SendIdentify { peer_id } => {
                debug!("🔄 [IdentifyHandler] Processing SendIdentify command for peer: {:?}", peer_id);
                // Identify protocol automatically sends identify information
                info!("📤 [IdentifyHandler] Identify information will be sent automatically to peer: {:?}", peer_id);
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            identify::Event::Received { peer_id, info, connection_id: _ } => {
                info!(
                    "📨 [IdentifyHandler] Identify information received - Peer: {:?}, Info: {:?}",
                    peer_id, info
                );
            }
            identify::Event::Sent { peer_id, connection_id: _ } => {
                debug!(
                    "📤 [IdentifyHandler] Identify information sent to peer: {:?}",
                    peer_id
                );
            }
            identify::Event::Pushed { peer_id, connection_id: _, info: _ } => {
                debug!(
                    "📤 [IdentifyHandler] Identify information pushed to peer: {:?}",
                    peer_id
                );
            }
            identify::Event::Error { peer_id, error, connection_id: _ } => {
                debug!(
                    "❌ [IdentifyHandler] Identify error with peer {:?}: {}",
                    peer_id, error
                );
            }
        }
    }
}

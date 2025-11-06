//! Handler for XAuth behaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use xauth::behaviours::PorAuthBehaviour;
use tracing::{debug, info};

use super::command::XAuthCommand;

/// Handler for XAuth behaviour
#[derive(Default)]
pub struct XAuthHandler;

#[async_trait]
impl BehaviourHandler for XAuthHandler {
    type Behaviour = PorAuthBehaviour;
    type Event = xauth::events::PorAuthEvent;
    type Command = XAuthCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            XAuthCommand::StartAuth { peer_id } => {
                debug!("🔄 [XAuthHandler] Processing StartAuth command for peer: {:?}", peer_id);
                // Note: XAuth automatically handles authentication on connection
                info!("🔐 [XAuthHandler] Authentication will be handled automatically for peer: {:?}", peer_id);
            }
            XAuthCommand::ApproveAuth { peer_id } => {
                debug!("🔄 [XAuthHandler] Processing ApproveAuth command for peer: {:?}", peer_id);
                // Note: XAuth automatically approves/rejects based on PoR
                info!("✅ [XAuthHandler] Authentication approved for peer: {:?}", peer_id);
            }
            XAuthCommand::RejectAuth { peer_id } => {
                debug!("🔄 [XAuthHandler] Processing RejectAuth command for peer: {:?}", peer_id);
                // Note: XAuth automatically approves/rejects based on PoR
                info!("❌ [XAuthHandler] Authentication rejected for peer: {:?}", peer_id);
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            xauth::events::PorAuthEvent::MutualAuthSuccess { peer_id, connection_id, address, metadata } => {
                info!(
                    "✅ [XAuthHandler] Mutual authentication successful - Peer: {:?}, Connection: {:?}, Address: {}",
                    peer_id, connection_id, address
                );
            }
            xauth::events::PorAuthEvent::OutboundAuthSuccess { peer_id, connection_id, address, metadata } => {
                debug!(
                    "✅ [XAuthHandler] Outbound authentication successful - Peer: {:?}, Connection: {:?}, Address: {}",
                    peer_id, connection_id, address
                );
            }
            xauth::events::PorAuthEvent::InboundAuthSuccess { peer_id, connection_id, address } => {
                debug!(
                    "✅ [XAuthHandler] Inbound authentication successful - Peer: {:?}, Connection: {:?}, Address: {}",
                    peer_id, connection_id, address
                );
            }
            xauth::events::PorAuthEvent::OutboundAuthFailure { peer_id, connection_id, address, reason } => {
                debug!(
                    "❌ [XAuthHandler] Outbound authentication failed - Peer: {:?}, Connection: {:?}, Address: {}, Reason: {}",
                    peer_id, connection_id, address, reason
                );
            }
            xauth::events::PorAuthEvent::InboundAuthFailure { peer_id, connection_id, address, reason } => {
                debug!(
                    "❌ [XAuthHandler] Inbound authentication failed - Peer: {:?}, Connection: {:?}, Address: {}, Reason: {}",
                    peer_id, connection_id, address, reason
                );
            }
            xauth::events::PorAuthEvent::AuthTimeout { peer_id, connection_id, address, direction } => {
                debug!(
                    "⏰ [XAuthHandler] Authentication timeout - Peer: {:?}, Connection: {:?}, Address: {}, Direction: {:?}",
                    peer_id, connection_id, address, direction
                );
            }
            xauth::events::PorAuthEvent::VerifyPorRequest { peer_id, connection_id, address, por, metadata } => {
                debug!(
                    "📋 [XAuthHandler] PoR verification requested - Peer: {:?}, Connection: {:?}, Address: {}",
                    peer_id, connection_id, address
                );
            }
        }
    }
}

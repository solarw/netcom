//! Handler for PingBehaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use libp2p::ping;
use tracing::{debug, info};

use super::command::PingCommand;

/// Handler for PingBehaviour
#[derive(Default)]
pub struct PingHandler;

#[async_trait]
impl BehaviourHandler for PingHandler {
    type Behaviour = ping::Behaviour;
    type Event = ping::Event;
    type Command = PingCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            PingCommand::SendPing { peer_id } => {
                debug!("🔄 [PingHandler] Processing SendPing command for peer: {:?}", peer_id);
                // Note: Ping protocol doesn't have explicit send method in libp2p
                // The protocol automatically sends pings to connected peers
                info!("📡 [PingHandler] Ping will be sent automatically to connected peer: {:?}", peer_id);
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            ping::Event { peer, result, connection: _ } => {
                match result {
                    Ok(rtt) => {
                        debug!(
                            "🏓 [PingHandler] Ping successful - Peer: {:?}, RTT: {:?}",
                            peer, rtt
                        );
                    }
                    Err(error) => {
                        debug!(
                            "❌ [PingHandler] Ping failed - Peer: {:?}, Error: {}",
                            peer, error
                        );
                    }
                }
            }
        }
    }
}

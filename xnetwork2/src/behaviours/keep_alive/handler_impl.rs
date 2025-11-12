//! BehaviourHandler implementation for KeepAliveBehaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use tracing::{debug, info};

use super::behaviour::{KeepAliveBehaviour, KeepAliveEvent};
use super::command::KeepAliveCommand;

/// Handler for KeepAliveBehaviour
#[derive(Default)]
pub struct KeepAliveHandler;

#[async_trait]
impl BehaviourHandler for KeepAliveHandler {
    type Behaviour = KeepAliveBehaviour;
    type Event = KeepAliveEvent;
    type Command = KeepAliveCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            KeepAliveCommand::SetKeepAlive { enabled, response } => {
                debug!("🔄 [KeepAliveHandler] Setting keep-alive to: {}", enabled);
                behaviour.set_enabled(enabled);
                info!("✅ [KeepAliveHandler] Keep-alive set to: {}", enabled);
                let _ = response.send(Ok(()));
            }
            KeepAliveCommand::GetKeepAliveStatus { response } => {
                debug!("🔄 [KeepAliveHandler] Getting keep-alive status");
                let status = behaviour.is_enabled();
                info!("📊 [KeepAliveHandler] Keep-alive status: {}", status);
                let _ = response.send(Ok(status));
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            KeepAliveEvent::Dummy => {
                // No events to handle for KeepAliveBehaviour
            }
        }
    }
}

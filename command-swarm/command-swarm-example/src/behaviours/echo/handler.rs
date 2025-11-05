//! Handler for EchoBehaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;

use super::behaviour::EchoBehaviour;
use super::command::EchoCommand;
use super::event::EchoEvent;

/// Handler for EchoBehaviour
#[derive(Default)]
pub struct EchoBehaviourHandler;

#[async_trait]
impl BehaviourHandler for EchoBehaviourHandler {
    type Behaviour = EchoBehaviour;
    type Event = EchoEvent;
    type Command = EchoCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            EchoCommand::SendMessage { peer_id, text } => {
                println!(
                    "🔄 [EchoHandler] Processing SendMessage command - Peer: {:?}, Text: {}",
                    peer_id, text
                );
                // Call behaviour method via command
                behaviour.send_message(peer_id, text);
                println!("📤 [EchoHandler] Command executed, event added to Swarm queue");
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            EchoEvent::MessageReceived { peer_id, text } => {
                println!(
                    "📨 [EchoHandler] Message received - From: {:?}, Content: {}",
                    peer_id, text
                );
            }
        }
    }
}

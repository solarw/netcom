//! Handler for PingBehaviour

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use libp2p::ping;

use super::command::PingCommand;

/// Handler for PingBehaviour
#[derive(Default)]
pub struct PingBehaviourHandler;

#[async_trait]
impl BehaviourHandler for PingBehaviourHandler {
    type Behaviour = ping::Behaviour;
    type Event = ping::Event;
    type Command = PingCommand;

    async fn handle_cmd(&mut self, _behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            PingCommand::DummyTest => {
                println!("🔄 [PingHandler] Processing DummyTest command");
            }
        }
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        // Can log ping events, but not required by the task
        match event {
            ping::Event {
                peer,
                result,
                connection: _,
            } => {
                println!("📡 [PingHandler] Ping event - Peer: {:?}, Result: {:?}", peer, result);
            }
        }
    }
}

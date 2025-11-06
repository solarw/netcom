//! Commands for Echo behaviour

use libp2p::PeerId;
use std::error::Error;
use tokio::sync::oneshot;

/// Commands for echo behaviour
#[derive(Debug)]
pub enum EchoCommand {
    /// Send message to peer
    SendMessage { 
        peer_id: PeerId, 
        text: String,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>
    },
}

impl command_swarm::SwarmCommand for EchoCommand {
    type Output = ();
}

//! Commands for Echo behaviour

use libp2p::PeerId;

/// Commands for echo behaviour - fully automatic generation
command_swarm::swarm_commands! {
    EchoCommand {
        SendMessage(peer_id: PeerId, text: String) -> (),
    }

}

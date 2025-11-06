//! Main behaviour for XNetwork2 using command-swarm macro


use crate::behaviours::{
    IdentifyHandler, PingHandler, XAuthHandler, XStreamHandler
};
use crate::swarm_commands::SwarmLevelCommand;
use crate::swarm_handler::XNetworkSwarmHandler;
use command_swarm::{
    BehaviourHandlerDispatcherTrait, SwarmHandler, SwarmLoopBuilder, make_command_swarm,
};


// Generate the complete command-swarm infrastructure
make_command_swarm! {
    behaviour_name: XNetworkBehaviour,
    behaviours_handlers: {
        identify: IdentifyHandler,
        ping: PingHandler,
        xauth: XAuthHandler,
        xstream: XStreamHandler
    },
    commands: {
        name: XNetworkCommands,
        swarm_level: SwarmLevelCommand
    },
    swarm_handler: XNetworkSwarmHandler
}

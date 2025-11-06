//! XNetwork2 - P2P сеть на базе command-swarm
//! 
//! Переработанная версия xnetwork с использованием архитектуры command-swarm
//! для структурированного управления libp2p swarm через отдельные handlers.

#![allow(warnings)]

pub mod behaviours;
pub mod swarm_commands;
pub mod swarm_handler;
pub mod main_behaviour;
pub mod node;
pub mod commander;
pub mod node_events;

// Re-export main components for public API
pub use behaviours::*;
pub use swarm_commands::SwarmLevelCommand;
pub use swarm_handler::XNetworkSwarmHandler;
pub use main_behaviour::{XNetworkBehaviour, XNetworkCommands, XNetworkBehaviourHandlerDispatcher};
pub use node::Node;
pub use commander::Commander;

// Re-export commonly used types
pub use libp2p::{Multiaddr, PeerId};
pub use command_swarm::{SwarmLoop, SwarmLoopBuilder, SwarmLoopStopper};

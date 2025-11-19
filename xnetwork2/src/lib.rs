//! XNetwork2 - P2P сеть на базе command-swarm
//!
//! Переработанная версия xnetwork с использованием архитектуры command-swarm
//! для структурированного управления libp2p swarm через отдельные handlers.

#![allow(warnings)]

pub mod behaviours;
pub mod commander;
pub mod connection_tracker;
pub mod connection_tracker_commands;
pub mod main_behaviour;
pub mod node;
pub mod node_builder;
pub mod node_events;
pub mod swarm_commands;
pub mod swarm_handler;

// Re-export main components for public API
pub use behaviours::*;
pub use commander::Commander;
pub use main_behaviour::{XNetworkBehaviour, XNetworkBehaviourHandlerDispatcher, XNetworkCommands};
pub use node::Node;
pub use node_builder::{InboundDecisionPolicy, NodeBuilder, builder};
pub use swarm_commands::SwarmLevelCommand;
pub use swarm_handler::XNetworkSwarmHandler;

// Re-export commonly used types
pub use command_swarm::{SwarmLoop, SwarmLoopBuilder, SwarmLoopStopper};
pub use libp2p::{Multiaddr, PeerId};

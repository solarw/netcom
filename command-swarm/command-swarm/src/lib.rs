//! Command-Swarm: A library for building libp2p applications with command-based swarm management
//!
//! This library provides traits and utilities for managing libp2p swarms through
//! command-based interfaces, making it easier to build complex p2p applications.

pub mod handlers;
pub mod macros;
pub mod swarm_loop;

pub use handlers::{BehaviourHandler, SwarmHandler};
pub use swarm_loop::{BehaviourHandlerDispatcherTrait, SwarmLoop, SwarmLoopBuilder, SwarmLoopStopper};

/// Re-export commonly used libp2p types for convenience
pub use libp2p::{
    Multiaddr, PeerId, Swarm,
    swarm::{ConnectionId, NetworkBehaviour, SwarmEvent},
};

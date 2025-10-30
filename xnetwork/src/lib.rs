// src/lib.rs

#![allow(warnings)]

pub mod behaviour;
pub mod commander;
pub mod commands;
pub mod connection_management;
pub mod events;
pub mod node;
pub mod node_builder;
pub mod utils;
pub mod xroutes;

// Re-export main components for public API
pub use behaviour::{make_behaviour, make_behaviour_with_config, NodeBehaviour};
pub use commander::Commander;
pub use commands::NetworkCommand;
pub use connection_management::{ConnectionInfo, PeerInfo, NetworkState, AuthStatus, ConnectionDirection, ConnectionState};
pub use events::NetworkEvent;
pub use node::NetworkNode;
pub use node_builder::NodeBuilder;
pub use utils::make_new_key;

// Re-export XRoutes components
pub use xroutes::{
    XRoutesConfig, 
    XRoutesCommand, 
    XRoutesEvent, 
    XRouteRole, 
    XRoutesCommander, 
    BootstrapNodeInfo, 
    BootstrapError,
    XRoutesDiscoveryBehaviour,
    XRoutesDiscoveryBehaviourEvent,
    XRoutesHandler,
};

// Re-export XRoutes behaviour components for advanced usage
pub use xroutes::behaviour::KadStats;

// Re-export protocol constants
pub use xroutes::{XROUTE_CLIENT_PROTOCOL, XROUTE_SERVER_PROTOCOL};

// src/xroutes/mod.rs

pub mod behaviour;
pub mod commands;
pub mod commander;
pub mod events;
pub mod handler;
pub mod types;
pub mod xroute;

pub use behaviour::{XRoutesDiscoveryBehaviour, XRoutesDiscoveryBehaviourEvent};
pub use commands::XRoutesCommand;
pub use commander::XRoutesCommander;
pub use events::XRoutesEvent;
pub use handler::XRoutesHandler;
pub use types::{BootstrapNodeInfo, BootstrapError};
pub use xroute::{XRouteRole, XROUTE_CLIENT_PROTOCOL, XROUTE_SERVER_PROTOCOL};

#[derive(Debug, Clone)]
pub struct XRoutesConfig {
    pub enable_mdns: bool,
    pub enable_kad: bool,
    pub kad_server_mode: bool,
    pub initial_role: XRouteRole,
}
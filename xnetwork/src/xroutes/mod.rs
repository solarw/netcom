// src/xroutes/mod.rs

pub mod behaviour;
pub mod commands;
pub mod commander;
pub mod events;
pub mod handler;
pub mod types;
pub mod xroute;
pub mod discovery;
pub mod connectivity;

// Re-export main XRoutes components
pub use behaviour::{XRoutesDiscoveryBehaviour, XRoutesDiscoveryBehaviourEvent, KadStats};
pub use commands::XRoutesCommand;
pub use commander::XRoutesCommander;
pub use events::XRoutesEvent;
pub use handler::XRoutesHandler;
pub use types::{BootstrapNodeInfo, BootstrapError};
pub use xroute::{XRouteRole, XROUTE_CLIENT_PROTOCOL, XROUTE_SERVER_PROTOCOL};
pub use connectivity::{ConnectivityBehaviour, ConnectivityCommand, ConnectivityEvent};

/// Configuration for XRoutes discovery system
#[derive(Debug, Clone)]
pub struct XRoutesConfig {
    /// Enable mDNS local discovery
    pub enable_mdns: bool,
    /// Enable Kademlia DHT discovery
    pub enable_kad: bool,
    /// Run Kademlia in server mode (for bootstrap nodes)
    pub kad_server_mode: bool,
    /// Initial role for this node
    pub initial_role: XRouteRole,
}

impl Default for XRoutesConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: true,
            kad_server_mode: false,
            initial_role: XRouteRole::default(),
        }
    }
}

impl XRoutesConfig {
    /// Create a new XRoutes configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable mDNS discovery
    pub fn with_mdns(mut self, enable: bool) -> Self {
        self.enable_mdns = enable;
        self
    }

    /// Enable Kademlia DHT discovery
    pub fn with_kad(mut self, enable: bool) -> Self {
        self.enable_kad = enable;
        self
    }

    /// Set Kademlia server mode (for bootstrap nodes)
    pub fn with_kad_server_mode(mut self, server_mode: bool) -> Self {
        self.kad_server_mode = server_mode;
        self
    }

    /// Set initial XRoute role
    pub fn with_role(mut self, role: XRouteRole) -> Self {
        self.initial_role = role;
        self
    }

    /// Create configuration for a bootstrap server
    pub fn bootstrap_server() -> Self {
        Self {
            enable_mdns: false, // Bootstrap servers typically don't need mDNS
            enable_kad: true,
            kad_server_mode: true,
            initial_role: XRouteRole::Server,
        }
    }

    /// Create configuration for a regular client
    pub fn client() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: true,
            kad_server_mode: false,
            initial_role: XRouteRole::Client,
        }
    }

    /// Create configuration with only local discovery (no DHT)
    pub fn local_only() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: false,
            kad_server_mode: false,
            initial_role: XRouteRole::Client,
        }
    }

    /// Create configuration with DHT only (no local discovery)
    pub fn dht_only() -> Self {
        Self {
            enable_mdns: false,
            enable_kad: true,
            kad_server_mode: false,
            initial_role: XRouteRole::Client,
        }
    }

    /// Disable all discovery mechanisms
    pub fn disabled() -> Self {
        Self {
            enable_mdns: false,
            enable_kad: false,
            kad_server_mode: false,
            initial_role: XRouteRole::Unknown,
        }
    }

    /// Check if any discovery mechanism is enabled
    pub fn is_discovery_enabled(&self) -> bool {
        self.enable_mdns || self.enable_kad
    }

    /// Check if XRoutes should be enabled (discovery features)
    pub fn is_xroutes_enabled(&self) -> bool {
        self.is_discovery_enabled()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.kad_server_mode && !self.enable_kad {
            return Err("Cannot enable Kademlia server mode without enabling Kademlia".to_string());
        }

        if self.kad_server_mode && self.initial_role != XRouteRole::Server {
            return Err("Kademlia server mode requires Server role".to_string());
        }

        Ok(())
    }
}

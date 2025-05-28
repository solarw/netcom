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
pub use connectivity::{ConnectivityBehaviour, ConnectivityCommand, ConnectivityEvent, RelayClientStats, RelayServerStats};

/// Configuration for XRoutes discovery and connectivity system
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
    /// Enable relay client functionality
    pub enable_relay_client: bool,
    /// Enable relay server functionality
    pub enable_relay_server: bool,
    /// Known relay server addresses (for explicit connections)
    pub known_relay_servers: Vec<libp2p::Multiaddr>,
}

impl Default for XRoutesConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: true,
            kad_server_mode: false,
            initial_role: XRouteRole::default(),
            enable_relay_client: true,
            enable_relay_server: false,
            known_relay_servers: Vec::new(),
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

    /// Enable relay client functionality
    pub fn with_relay_client(mut self, enable: bool) -> Self {
        self.enable_relay_client = enable;
        self
    }

    /// Enable relay server functionality
    pub fn with_relay_server(mut self, enable: bool) -> Self {
        self.enable_relay_server = enable;
        self
    }

    /// Add known relay server addresses
    pub fn with_relay_servers(mut self, servers: Vec<libp2p::Multiaddr>) -> Self {
        self.known_relay_servers = servers;
        self
    }

    /// Add a single relay server address
    pub fn with_relay_server_addr(mut self, server: libp2p::Multiaddr) -> Self {
        self.known_relay_servers.push(server);
        self
    }

    /// Create configuration for a bootstrap server
    pub fn bootstrap_server() -> Self {
        Self {
            enable_mdns: false, // Bootstrap servers typically don't need mDNS
            enable_kad: true,
            kad_server_mode: true,
            initial_role: XRouteRole::Server,
            enable_relay_client: true,
            enable_relay_server: false,
            known_relay_servers: Vec::new(),
        }
    }

    /// Create configuration for a regular client
    pub fn client() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: true,
            kad_server_mode: false,
            initial_role: XRouteRole::Client,
            enable_relay_client: true,
            enable_relay_server: false,
            known_relay_servers: Vec::new(),
        }
    }

    /// Create configuration with only local discovery (no DHT)
    pub fn local_only() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: false,
            kad_server_mode: false,
            initial_role: XRouteRole::Client,
            enable_relay_client: true,
            enable_relay_server: false,
            known_relay_servers: Vec::new(),
        }
    }

    /// Create configuration with DHT only (no local discovery)
    pub fn dht_only() -> Self {
        Self {
            enable_mdns: false,
            enable_kad: true,
            kad_server_mode: false,
            initial_role: XRouteRole::Client,
            enable_relay_client: true,
            enable_relay_server: false,
            known_relay_servers: Vec::new(),
        }
    }

    /// Disable all discovery mechanisms
    pub fn disabled() -> Self {
        Self {
            enable_mdns: false,
            enable_kad: false,
            kad_server_mode: false,
            initial_role: XRouteRole::Unknown,
            enable_relay_client: false,
            enable_relay_server: false,
            known_relay_servers: Vec::new(),
        }
    }

    /// Check if any discovery mechanism is enabled
    pub fn is_discovery_enabled(&self) -> bool {
        self.enable_mdns || self.enable_kad
    }

    /// Check if XRoutes should be enabled (discovery or connectivity features)
    pub fn is_xroutes_enabled(&self) -> bool {
        self.is_discovery_enabled() || self.enable_relay_client || self.enable_relay_server
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

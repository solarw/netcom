//! Types for XRoutes behaviour

use libp2p::StreamProtocol;

/// Configuration for XRoutes behaviour
#[derive(Debug, Clone)]
pub struct XRoutesConfig {
    /// Enable identify behaviour with custom protocol
    pub enable_identify: bool,
    /// Enable mDNS local discovery
    pub enable_mdns: bool,
    /// Enable Kademlia DHT discovery
    pub enable_kad: bool,
    /// Enable relay server behaviour
    pub enable_relay_server: bool,
    // relay_client теперь всегда включен, поэтому enable_relay_client убран
}

impl Default for XRoutesConfig {
    fn default() -> Self {
        Self {
            enable_identify: true,
            enable_mdns: true,
            enable_kad: true,
            enable_relay_server: false,
        }
    }
}

impl XRoutesConfig {
    /// Create a new XRoutes configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable identify behaviour
    pub fn with_identify(mut self, enable: bool) -> Self {
        self.enable_identify = enable;
        self
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

    /// Create configuration with all behaviours disabled
    pub fn disabled() -> Self {
        Self {
            enable_identify: false,
            enable_mdns: false,
            enable_kad: false,
            enable_relay_server: false,
        }
    }

    /// Enable relay server behaviour
    pub fn with_relay_server(mut self, enable: bool) -> Self {
        self.enable_relay_server = enable;
        self
    }
}

/// Status of XRoutes behaviours
#[derive(Debug, Clone)]
pub struct XRoutesStatus {
    /// Identify behaviour status
    pub identify_enabled: bool,
    /// mDNS behaviour status
    pub mdns_enabled: bool,
    /// Kademlia behaviour status
    pub kad_enabled: bool,
    /// Relay server behaviour status
    pub relay_server_enabled: bool,
    /// Relay client behaviour status
    pub relay_client_enabled: bool,
}

impl Default for XRoutesStatus {
    fn default() -> Self {
        Self {
            identify_enabled: false,
            mdns_enabled: false,
            kad_enabled: false,
            relay_server_enabled: false,
            relay_client_enabled: false,
        }
    }
}

/// Custom protocol for XRoutes identify
pub const XROUTES_IDENTIFY_PROTOCOL: StreamProtocol = StreamProtocol::new("/xroutes/identify");

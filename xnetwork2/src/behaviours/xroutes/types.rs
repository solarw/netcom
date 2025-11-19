//! Types for XRoutes behaviour

use libp2p::StreamProtocol;

/// Configuration for XRoutes behaviour
#[derive(Debug, Clone)]
pub struct XRoutesConfig {
    /// Enable identify behaviour with custom protocol
    pub enable_identify: bool,
    /// Enable mDNS local discovery
    pub enable_mdns: bool,
    /// Enable Kademlia DHT discovery (legacy, use kad_mode instead)
    pub enable_kad: bool,
    /// Kademlia mode configuration
    pub kad_mode: Option<KadMode>,
    /// Enable relay server behaviour
    pub enable_relay_server: bool,
    // relay_client теперь всегда включен, поэтому enable_relay_client убран
    /// Enable DCUtR for hole punching
    pub enable_dcutr: bool,
    /// Enable AutoNAT server for providing NAT detection services
    pub enable_autonat_server: bool,
    /// Enable AutoNAT client for detecting NAT type
    pub enable_autonat_client: bool,
    /// Enable automatic hole punching
    pub auto_hole_punching: bool,
}

impl Default for XRoutesConfig {
    fn default() -> Self {
        Self {
            enable_identify: true,
            enable_mdns: true,
            enable_kad: true,
            kad_mode: None,
            enable_relay_server: false,
            enable_dcutr: false,
            enable_autonat_server: false,
            enable_autonat_client: false,
            auto_hole_punching: false,
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

    /// Set Kademlia mode
    pub fn with_kad_mode(mut self, mode: KadMode) -> Self {
        self.kad_mode = Some(mode);
        self.enable_kad = true;
        self
    }

    /// Enable Kademlia server mode
    pub fn with_kad_server(mut self) -> Self {
        self.kad_mode = Some(KadMode::Server);
        self.enable_kad = true;
        self
    }

    /// Enable Kademlia client mode
    pub fn with_kad_client(mut self) -> Self {
        self.kad_mode = Some(KadMode::Client);
        self.enable_kad = true;
        self
    }

    /// Create configuration with all behaviours disabled
    pub fn disabled() -> Self {
        Self {
            enable_identify: false,
            enable_mdns: false,
            enable_kad: false,
            kad_mode: None,
            enable_relay_server: false,
            enable_dcutr: false,
            enable_autonat_server: false,
            enable_autonat_client: false,
            auto_hole_punching: false,
        }
    }

    /// Enable relay server behaviour
    pub fn with_relay_server(mut self, enable: bool) -> Self {
        self.enable_relay_server = enable;
        self
    }

    /// Enable DCUtR for hole punching
    pub fn with_dcutr(mut self, enable: bool) -> Self {
        self.enable_dcutr = enable;
        self
    }


    /// Enable AutoNAT server for providing NAT detection services
    pub fn with_autonat_server(mut self, enable: bool) -> Self {
        self.enable_autonat_server = enable;
        self
    }

    /// Enable AutoNAT client for detecting NAT type
    pub fn with_autonat_client(mut self, enable: bool) -> Self {
        self.enable_autonat_client = enable;
        self
    }

    /// Enable automatic hole punching
    pub fn with_auto_hole_punching(mut self, enable: bool) -> Self {
        self.auto_hole_punching = enable;
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
    /// Kademlia mode (if enabled)
    pub kad_mode: Option<KadMode>,
    /// Relay server behaviour status
    pub relay_server_enabled: bool,
    /// Relay client behaviour status
    pub relay_client_enabled: bool,
    /// DCUtR behaviour status
    pub dcutr_enabled: bool,
    /// AutoNAT server behaviour status
    pub autonat_server_enabled: bool,
    /// AutoNAT client behaviour status
    pub autonat_client_enabled: bool,
}

impl Default for XRoutesStatus {
    fn default() -> Self {
        Self {
            identify_enabled: false,
            mdns_enabled: false,
            kad_enabled: false,
            kad_mode: None,
            relay_server_enabled: false,
            relay_client_enabled: false,
            dcutr_enabled: false,
            autonat_server_enabled: false,
            autonat_client_enabled: false,
        }
    }
}

/// Custom protocol for XRoutes identify
pub const XROUTES_IDENTIFY_PROTOCOL: StreamProtocol = StreamProtocol::new("/xroutes/identify");

/// Kademlia mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KadMode {
    /// Client mode - only makes requests, does not respond to other peers
    Client,
    /// Server mode - responds to requests from other peers
    Server,
    /// Auto mode - automatically switches between client/server based on external addresses
    Auto,
}

impl std::fmt::Display for KadMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KadMode::Client => write!(f, "client"),
            KadMode::Server => write!(f, "server"),
            KadMode::Auto => write!(f, "auto"),
        }
    }
}

impl std::str::FromStr for KadMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "client" => Ok(KadMode::Client),
            "server" => Ok(KadMode::Server),
            "auto" => Ok(KadMode::Auto),
            _ => Err(format!("Invalid Kademlia mode: {}. Valid modes: client, server, auto", s)),
        }
    }
}

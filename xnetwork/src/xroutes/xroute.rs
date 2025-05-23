// src/xroutes/xroute.rs

use libp2p::StreamProtocol;

// XRoute protocol markers for role identification
pub const XROUTE_CLIENT_PROTOCOL: StreamProtocol = StreamProtocol::new("/xroutes/1.0/client");
pub const XROUTE_SERVER_PROTOCOL: StreamProtocol = StreamProtocol::new("/xroutes/1.0/server");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XRouteRole {
    Client,
    Server,
    Unknown,
}

impl XRouteRole {
    /// Convert role to its corresponding protocol marker
    pub fn to_protocol(&self) -> Option<StreamProtocol> {
        match self {
            XRouteRole::Client => Some(XROUTE_CLIENT_PROTOCOL),
            XRouteRole::Server => Some(XROUTE_SERVER_PROTOCOL),
            XRouteRole::Unknown => None,
        }
    }
    
    /// Determine role from a list of protocols
    pub fn from_protocols(protocols: &[StreamProtocol]) -> XRouteRole {
        if protocols.contains(&XROUTE_SERVER_PROTOCOL) {
            XRouteRole::Server
        } else if protocols.contains(&XROUTE_CLIENT_PROTOCOL) {
            XRouteRole::Client
        } else {
            XRouteRole::Unknown
        }
    }
    
    /// Get role as string for logging
    pub fn as_str(&self) -> &'static str {
        match self {
            XRouteRole::Client => "client",
            XRouteRole::Server => "server",
            XRouteRole::Unknown => "unknown",
        }
    }
}

impl Default for XRouteRole {
    fn default() -> Self {
        XRouteRole::Client
    }
}
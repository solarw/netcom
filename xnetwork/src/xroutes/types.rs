// src/xroutes/types.rs

use libp2p::PeerId;
use super::xroute::XRouteRole;

// Bootstrap connection types
#[derive(Debug, Clone)]
pub struct BootstrapNodeInfo {
    pub peer_id: PeerId,
    pub role: XRouteRole,
    pub protocols: Vec<String>,
    pub agent_version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Connection timed out")]
    ConnectionTimeout,
    
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    
    #[error("Role check failed: {0}")]
    RoleCheckFailed(String),
    
    #[error("Not a bootstrap server, found role: {0:?}")]
    NotABootstrapServer(XRouteRole),
    
    #[error("XRoutes discovery not enabled")]
    DiscoveryNotEnabled,
}
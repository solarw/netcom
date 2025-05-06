use std::{collections::HashMap, time::{Duration, Instant}};

use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};

use super::por::por::ProofOfRepresentation;


// Protocol identifier
pub const PROTOCOL_ID: &str = "/por-auth/1.0.0";
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(5);

// Auth verification tracking struct - new
pub struct PendingVerification {
    pub peer_id: PeerId, 
    pub address: Multiaddr,
    pub por: ProofOfRepresentation,
    pub metadata: HashMap<String, String>,
    pub response_channel: ResponseChannel<PorAuthResponse>,
    pub received_at: Instant,
}

// Authentication messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PorAuthRequest {
    pub por: ProofOfRepresentation,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PorAuthResponse {
    pub result: AuthResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthResult {
    Ok(HashMap<String, String>),
    Error(String),
}


#[derive(Debug, Clone)]
pub enum AuthDirection {
    Inbound,  // From remote peer to us
    Outbound, // From us to remote peer
    Both,     // In both directions
}

// New enum types for individual authentication directions
#[derive(Debug, Clone, PartialEq)]
pub enum DirectionalAuthState {
    // Not yet started auth process in this direction
    NotStarted,
    // Authentication in progress
    InProgress { started: Instant },
    // Authentication succeeded
    Successful(HashMap<String, String>),
    // Authentication failed
    Failed(String),
}

// Compute combined authentication state
#[derive(Debug, Clone, PartialEq)]
pub enum CombinedAuthState {
    // Not authenticated in either direction
    NotAuthenticated,
    // Only outbound authenticated
    OutboundOnly(HashMap<String, String>),
    // Only inbound authenticated
    InboundOnly,
    // Full mutual authentication
    FullyAuthenticated(HashMap<String, String>),
    // Authentication has failed
    Failed(String),
}

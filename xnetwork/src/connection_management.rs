// src/connection_management.rs

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId, StreamProtocol};
use std::time::{Duration, Instant};

/// Direction of the connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDirection {
    Inbound,  // We received the connection
    Outbound, // We initiated the connection
}

/// Status of authentication for a connection/peer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStatus {
    NotStarted,     // Authentication not initiated
    InProgress,     // Authentication in progress
    Authenticated,  // Successfully authenticated
    Failed(String), // Authentication failed with reason
    Revoked,        // Authentication was revoked
}

/// State of a connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,   // Connection being established
    Connected,    // Connection established
    Disconnecting, // Connection being closed
    Disconnected, // Connection closed
}

/// Statistics for a connection
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub streams_opened: u32,
    pub streams_closed: u32,
    pub last_activity: Option<Instant>,
}

/// Information about a single connection
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub connection_id: ConnectionId,
    pub peer_id: PeerId,
    pub remote_addr: Multiaddr,
    pub local_addr: Option<Multiaddr>,
    pub direction: ConnectionDirection,
    pub established_at: Instant,
    pub protocols: Vec<StreamProtocol>,
    pub auth_status: AuthStatus,
    pub connection_state: ConnectionState,
    pub stats: ConnectionStats,
}

impl ConnectionInfo {
    pub fn new(
        connection_id: ConnectionId,
        peer_id: PeerId,
        remote_addr: Multiaddr,
        local_addr: Option<Multiaddr>,
        direction: ConnectionDirection,
    ) -> Self {
        Self {
            connection_id,
            peer_id,
            remote_addr,
            local_addr,
            direction,
            established_at: Instant::now(),
            protocols: Vec::new(),
            auth_status: AuthStatus::NotStarted,
            connection_state: ConnectionState::Connected,
            stats: ConnectionStats::default(),
        }
    }

    /// Check if this connection is currently active
    pub fn is_active(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Connected)
    }

    /// Check if this connection is authenticated
    pub fn is_authenticated(&self) -> bool {
        matches!(self.auth_status, AuthStatus::Authenticated)
    }

    /// Get connection duration
    pub fn duration(&self) -> Duration {
        self.established_at.elapsed()
    }
}

/// Information about a peer with all its connections
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub connections: Vec<ConnectionInfo>,
    pub auth_status: AuthStatus,
    pub first_connected_at: Instant,
    pub last_activity: Instant,
    pub total_connections: u32, // Total connections ever made
    pub is_authenticated: bool,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            connections: Vec::new(),
            auth_status: AuthStatus::NotStarted,
            first_connected_at: now,
            last_activity: now,
            total_connections: 0,
            is_authenticated: false,
        }
    }

    /// Get active connections for this peer
    pub fn active_connections(&self) -> Vec<&ConnectionInfo> {
        self.connections.iter().filter(|c| c.is_active()).collect()
    }

    /// Check if peer has any active connections
    pub fn is_connected(&self) -> bool {
        !self.active_connections().is_empty()
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.active_connections().len()
    }

    /// Get total bytes sent/received across all connections
    pub fn total_stats(&self) -> ConnectionStats {
        let mut total = ConnectionStats::default();
        for conn in &self.connections {
            total.bytes_sent += conn.stats.bytes_sent;
            total.bytes_received += conn.stats.bytes_received;
            total.streams_opened += conn.stats.streams_opened;
            total.streams_closed += conn.stats.streams_closed;
            if let Some(activity) = conn.stats.last_activity {
                if total.last_activity.is_none() || activity > total.last_activity.unwrap() {
                    total.last_activity = Some(activity);
                }
            }
        }
        total
    }

    /// Update peer activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Add a connection to this peer
    pub fn add_connection(&mut self, connection: ConnectionInfo) {
        self.total_connections += 1;
        self.update_activity();
        self.connections.push(connection);
    }

    /// Remove a connection from this peer
    pub fn remove_connection(&mut self, connection_id: ConnectionId) -> Option<ConnectionInfo> {
        if let Some(pos) = self.connections.iter().position(|c| c.connection_id == connection_id) {
            self.update_activity();
            Some(self.connections.remove(pos))
        } else {
            None
        }
    }

    /// Update authentication status
    pub fn update_auth_status(&mut self, status: AuthStatus) {
        self.auth_status = status.clone();
        self.is_authenticated = matches!(status, AuthStatus::Authenticated);
        self.update_activity();
    }
}

/// Overall network state information
#[derive(Debug, Clone)]
pub struct NetworkState {
    pub local_peer_id: PeerId,
    pub listening_addresses: Vec<Multiaddr>,
    pub total_connections: usize,
    pub authenticated_peers: usize,
    pub active_streams: usize,
    pub uptime: Duration,
    pub created_at: Instant,
}

impl NetworkState {
    pub fn new(local_peer_id: PeerId) -> Self {
        let now = Instant::now();
        Self {
            local_peer_id,
            listening_addresses: Vec::new(),
            total_connections: 0,
            authenticated_peers: 0,
            active_streams: 0,
            uptime: Duration::default(),
            created_at: now,
        }
    }

    pub fn update_uptime(&mut self) {
        self.uptime = self.created_at.elapsed();
    }
}
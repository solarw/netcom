//! Conntracker service for tracking peer connections and addresses

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use libp2p::{
    PeerId, Multiaddr,
    swarm::{FromSwarm, behaviour::ConnectionEstablished, behaviour::ConnectionClosed, behaviour::AddressChange, behaviour::NewListenAddr, behaviour::ExternalAddrConfirmed, behaviour::ExternalAddrExpired},
};
use libp2p::core::ConnectedPoint;
use libp2p::swarm::ConnectionId;

/// Status of a connection
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Active,
    Closing,
    Closed,
}

/// Information about a single connection
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub connection_id: ConnectionId,
    pub peer_id: PeerId,
    pub local_addr: Multiaddr,
    pub remote_addr: Multiaddr,
    pub endpoint: ConnectedPoint,
    pub established_at: Instant,
    pub status: ConnectionStatus,
}

/// All connections and addresses for a specific peer
#[derive(Debug, Clone)]
pub struct PeerConnections {
    pub peer_id: PeerId,
    pub addresses: HashSet<Multiaddr>,
    pub connections: HashMap<ConnectionId, ConnectionInfo>,
}

impl PeerConnections {
    /// Create a new PeerConnections instance for a peer
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            addresses: HashSet::new(),
            connections: HashMap::new(),
        }
    }

    /// Add a connection to this peer
    pub fn add_connection(&mut self, connection_info: ConnectionInfo) {
        // Add connection
        self.connections.insert(connection_info.connection_id, connection_info);
        
        // Update addresses from the connection
        // Note: We'll update addresses separately from Identify events for better accuracy
    }

    /// Remove a connection from this peer
    pub fn remove_connection(&mut self, connection_id: &ConnectionId) -> Option<ConnectionInfo> {
        self.connections.remove(connection_id)
    }

    /// Add an address for this peer
    pub fn add_address(&mut self, address: Multiaddr) {
        self.addresses.insert(address);
    }

    /// Remove an address from this peer
    pub fn remove_address(&mut self, address: &Multiaddr) -> bool {
        self.addresses.remove(address)
    }

    /// Get all active connections for this peer
    pub fn get_connections(&self) -> Vec<&ConnectionInfo> {
        self.connections.values().collect()
    }

    /// Check if the peer has any active connections
    pub fn is_connected(&self) -> bool {
        !self.connections.is_empty()
    }

    /// Get the number of active connections
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

/// Statistics about connections
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_peers: usize,
    pub total_connections: usize,
    pub listen_addresses_count: usize,
    pub external_addresses_count: usize,
}

/// Conntracker service for tracking all peer connections and addresses
pub struct Conntracker {
    peer_connections: HashMap<PeerId, PeerConnections>,
    listen_addresses: Vec<Multiaddr>,
    external_addresses: Vec<Multiaddr>,
    local_peer_id: PeerId,
}

impl Conntracker {
    /// Create a new Conntracker
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            peer_connections: HashMap::new(),
            listen_addresses: Vec::new(),
            external_addresses: Vec::new(),
            local_peer_id,
        }
    }

    /// Get PeerConnections for a specific peer
    pub fn get_peer_connections(&self, peer_id: &PeerId) -> Option<&PeerConnections> {
        self.peer_connections.get(peer_id)
    }

    /// Get information about a specific connection
    pub fn get_connection(&self, connection_id: &ConnectionId) -> Option<&ConnectionInfo> {
        for peer_conn in self.peer_connections.values() {
            if let Some(conn_info) = peer_conn.connections.get(connection_id) {
                return Some(conn_info);
            }
        }
        None
    }

    /// Get all connected peers (peers with at least one active connection)
    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.peer_connections
            .iter()
            .filter(|(_, pc)| pc.is_connected())
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }

    /// Get all connections (active and inactive)
    pub fn get_all_connections(&self) -> Vec<&ConnectionInfo> {
        self.peer_connections
            .values()
            .flat_map(|pc| pc.connections.values())
            .collect()
    }

    /// Get listen addresses of the local node
    pub fn get_listen_addresses(&self) -> &[Multiaddr] {
        &self.listen_addresses
    }

    /// Get external addresses of the local node
    pub fn get_external_addresses(&self) -> &[Multiaddr] {
        &self.external_addresses
    }

    /// Get connection statistics
    pub fn get_connection_stats(&self) -> ConnectionStats {
        let total_connections = self.peer_connections
            .values()
            .map(|pc| pc.connection_count())
            .sum();
        
        ConnectionStats {
            total_peers: self.peer_connections.len(),
            total_connections,
            listen_addresses_count: self.listen_addresses.len(),
            external_addresses_count: self.external_addresses.len(),
        }
    }

    /// Handle ConnectionEstablished event
    pub fn handle_connection_established(&mut self, event: &ConnectionEstablished) {
        let connection_info = ConnectionInfo {
            connection_id: event.connection_id,
            peer_id: event.peer_id,
            local_addr: match &event.endpoint {
                ConnectedPoint::Dialer { address, .. } => address.clone(),
                ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.clone(),
            },
            remote_addr: match &event.endpoint {
                ConnectedPoint::Dialer { address, .. } => address.clone(),
                ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.clone(),
            },
            endpoint: event.endpoint.clone(),
            established_at: Instant::now(),
            status: ConnectionStatus::Active,
        };

        // Get or create PeerConnections for this peer
        let peer_connections = self.peer_connections
            .entry(event.peer_id)
            .or_insert_with(|| PeerConnections::new(event.peer_id));

        // Add the connection
        peer_connections.add_connection(connection_info);

        // Add addresses from the connection endpoint
        match &event.endpoint {
            ConnectedPoint::Dialer { address, .. } => {
                peer_connections.add_address(address.clone());
            }
            ConnectedPoint::Listener { send_back_addr, .. } => {
                peer_connections.add_address(send_back_addr.clone());
            }
        }
    }

    /// Handle ConnectionClosed event
    pub fn handle_connection_closed(&mut self, event: &ConnectionClosed) {
        if let Some(peer_connections) = self.peer_connections.get_mut(&event.peer_id) {
            if let Some(mut connection_info) = peer_connections.remove_connection(&event.connection_id) {
                // Update status to closed
                connection_info.status = ConnectionStatus::Closed;
                
                // If no more connections, we keep the peer entry for address tracking
                // but it will be filtered out by get_connected_peers()
            }
        }
    }

    /// Handle AddressChange event
    pub fn handle_address_change(&mut self, event: &AddressChange) {
        // Update connection addresses when they change
        if let Some(peer_connections) = self.peer_connections.get_mut(&event.peer_id) {
            // Remove old address
            peer_connections.remove_address(event.old.get_remote_address());
            // Add new address
            peer_connections.add_address(event.new.get_remote_address().clone());
        }
    }

    /// Handle NewListenAddr event
    pub fn handle_new_listen_addr(&mut self, event: &NewListenAddr) {
        self.listen_addresses.push(event.addr.clone());
    }

    /// Handle ExternalAddrConfirmed event
    pub fn handle_external_addr_confirmed(&mut self, event: &ExternalAddrConfirmed) {
        self.external_addresses.push(event.addr.clone());
    }

    /// Handle ExternalAddrExpired event
    pub fn handle_external_addr_expired(&mut self, event: &ExternalAddrExpired) {
        self.external_addresses.retain(|addr| addr != event.addr);
    }

    /// Add a listen address
    pub fn add_listen_address(&mut self, address: Multiaddr) {
        self.listen_addresses.push(address);
    }

    /// Remove a listen address
    pub fn remove_listen_address(&mut self, address: &Multiaddr) {
        self.listen_addresses.retain(|addr| addr != address);
    }

    /// Add an external address
    pub fn add_external_address(&mut self, address: Multiaddr) {
        self.external_addresses.push(address);
    }

    /// Add a connection
    pub fn add_connection(&mut self, connection_id: libp2p::swarm::ConnectionId, peer_id: PeerId, endpoint: libp2p::core::ConnectedPoint) {
        let connection_info = ConnectionInfo {
            connection_id,
            peer_id,
            local_addr: match &endpoint {
                libp2p::core::ConnectedPoint::Dialer { address, .. } => address.clone(),
                libp2p::core::ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.clone(),
            },
            remote_addr: match &endpoint {
                libp2p::core::ConnectedPoint::Dialer { address, .. } => address.clone(),
                libp2p::core::ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.clone(),
            },
            endpoint: endpoint.clone(),
            established_at: std::time::Instant::now(),
            status: ConnectionStatus::Active,
        };

        // Get or create PeerConnections for this peer
        let peer_connections = self.peer_connections
            .entry(peer_id)
            .or_insert_with(|| PeerConnections::new(peer_id));

        // Add the connection
        peer_connections.add_connection(connection_info);
    }

    /// Remove a connection
    pub fn remove_connection(&mut self, connection_id: &libp2p::swarm::ConnectionId) {
        for peer_connections in self.peer_connections.values_mut() {
            if let Some(mut connection_info) = peer_connections.remove_connection(connection_id) {
                // Update status to closed
                connection_info.status = ConnectionStatus::Closed;
            }
        }
    }

}

pub mod commands;

#[cfg(test)]
mod test_basic;

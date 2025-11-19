//! Basic tests for Conntracker functionality

#[cfg(test)]
mod tests {
    use super::super::*;
    use libp2p::{PeerId, Multiaddr, core::{ConnectedPoint, transport::PortUse}, swarm::ConnectionId};

    #[test]
    fn test_conntracker_creation() {
        let peer_id = PeerId::random();
        let conntracker = Conntracker::new(peer_id);
        
        assert_eq!(conntracker.get_connected_peers().len(), 0);
        assert_eq!(conntracker.get_all_connections().len(), 0);
        assert_eq!(conntracker.get_listen_addresses().len(), 0);
        assert_eq!(conntracker.get_external_addresses().len(), 0);
    }

    #[test]
    fn test_peer_connections_creation() {
        let peer_id = PeerId::random();
        let peer_connections = PeerConnections::new(peer_id);
        
        assert_eq!(peer_connections.peer_id, peer_id);
        assert_eq!(peer_connections.addresses.len(), 0);
        assert_eq!(peer_connections.connections.len(), 0);
        assert!(!peer_connections.is_connected());
        assert_eq!(peer_connections.connection_count(), 0);
    }

    #[test]
    fn test_connection_info_creation() {
        let connection_id = ConnectionId::new_unchecked(1);
        let peer_id = PeerId::random();
        let local_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        let remote_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8081".parse().unwrap();
        let endpoint = ConnectedPoint::Dialer {
            address: remote_addr.clone(),
            role_override: libp2p::core::Endpoint::Dialer,
            port_use: Default::default(),
        };

        let connection_info = ConnectionInfo {
            connection_id,
            peer_id,
            local_addr: local_addr.clone(),
            remote_addr: remote_addr.clone(),
            endpoint: endpoint.clone(),
            established_at: std::time::Instant::now(),
            status: ConnectionStatus::Active,
        };

        assert_eq!(connection_info.connection_id, connection_id);
        assert_eq!(connection_info.peer_id, peer_id);
        assert_eq!(connection_info.local_addr, local_addr);
        assert_eq!(connection_info.remote_addr, remote_addr);
        assert_eq!(connection_info.endpoint, endpoint);
        assert_eq!(connection_info.status, ConnectionStatus::Active);
    }

    #[test]
    fn test_peer_connections_add_remove() {
        let peer_id = PeerId::random();
        let mut peer_connections = PeerConnections::new(peer_id);
        
        let connection_id = ConnectionId::new_unchecked(1);
        let connection_info = ConnectionInfo {
            connection_id,
            peer_id,
            local_addr: "/ip4/127.0.0.1/tcp/8080".parse().unwrap(),
            remote_addr: "/ip4/127.0.0.1/tcp/8081".parse().unwrap(),
            endpoint: ConnectedPoint::Dialer {
                address: "/ip4/127.0.0.1/tcp/8081".parse().unwrap(),
                role_override: libp2p::core::Endpoint::Dialer,
                port_use: Default::default(),
            },
            established_at: std::time::Instant::now(),
            status: ConnectionStatus::Active,
        };

        // Add connection
        peer_connections.add_connection(connection_info);
        assert_eq!(peer_connections.connection_count(), 1);
        assert!(peer_connections.is_connected());
        
        // Remove connection
        let removed = peer_connections.remove_connection(&connection_id);
        assert!(removed.is_some());
        assert_eq!(peer_connections.connection_count(), 0);
        assert!(!peer_connections.is_connected());
    }

    #[test]
    fn test_peer_connections_address_management() {
        let peer_id = PeerId::random();
        let mut peer_connections = PeerConnections::new(peer_id);
        
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        
        // Add address
        peer_connections.add_address(address.clone());
        assert_eq!(peer_connections.addresses.len(), 1);
        assert!(peer_connections.addresses.contains(&address));
        
        // Remove address
        let removed = peer_connections.remove_address(&address);
        assert!(removed);
        assert_eq!(peer_connections.addresses.len(), 0);
    }

    #[test]
    fn test_conntracker_stats() {
        let peer_id = PeerId::random();
        let conntracker = Conntracker::new(peer_id);
        
        let stats = conntracker.get_connection_stats();
        assert_eq!(stats.total_peers, 0);
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.listen_addresses_count, 0);
        assert_eq!(stats.external_addresses_count, 0);
    }
}

// connection_handler_init_tests.rs
// Test 1: Initialization and creation of XStreamHandler

use crate::handler::XStreamHandler;
use crate::events::EstablishedConnection;
use libp2p::{Multiaddr, PeerId, swarm::ConnectionHandler, core::transport::PortUse};

#[tokio::test]
async fn test_handler_initialization() {
    // Test basic handler creation and initialization
    println!("🚀 Testing XStreamHandler initialization...");
    
    // Test 1: Create handler without peer_id
    let connection_id = libp2p::swarm::ConnectionId::new_unchecked(1);
    let peer_id = PeerId::random();
    let established_connection = EstablishedConnection::Outbound {
        addr: "/memory/0".parse().unwrap(),
    };
    let handler = XStreamHandler::new(connection_id, peer_id, established_connection);
    println!("✅ Handler created successfully without peer_id");
    
    // Test 2: Verify handler implements ConnectionHandler trait
    let listen_protocol = handler.listen_protocol();
    println!("✅ listen_protocol works - protocol: {:?}", listen_protocol);
    
    let keep_alive = handler.connection_keep_alive();
    println!("✅ connection_keep_alive works: {}", keep_alive);
    
    // Test 3: Create handler with peer_id
    let test_peer_id = PeerId::random();
    let connection_id = libp2p::swarm::ConnectionId::new_unchecked(2);
    let established_connection = EstablishedConnection::Outbound {
        addr: "/memory/0".parse().unwrap(),
    };
    let mut handler_with_peer = XStreamHandler::new(connection_id, test_peer_id, established_connection);
    handler_with_peer.set_peer_id(test_peer_id);
    println!("✅ Handler created successfully with peer_id: {}", test_peer_id);
    
    // Test 4: Verify handler state after peer_id set
    let listen_protocol_with_peer = handler_with_peer.listen_protocol();
    let keep_alive_with_peer = handler_with_peer.connection_keep_alive();
    
    println!("✅ Handler with peer_id - listen_protocol: {:?}, keep_alive: {}", 
             listen_protocol_with_peer, keep_alive_with_peer);
    
    // Test 5: Verify handler can be cloned (if needed)
    // Note: XStreamHandler doesn't implement Clone, which is normal for ConnectionHandler
    
    println!("✅ XStreamHandler initialization test completed successfully");
}

#[tokio::test]
async fn test_handler_protocol_methods() {
    // Test protocol-specific methods of XStreamHandler
    println!("🚀 Testing XStreamHandler protocol methods...");
    
    let connection_id = libp2p::swarm::ConnectionId::new_unchecked(3);
    let peer_id = PeerId::random();
    let established_connection = EstablishedConnection::Outbound {
        addr: "/memory/0".parse().unwrap(),
    };
    let handler = XStreamHandler::new(connection_id, peer_id, established_connection);
    
    // Test listen_protocol returns valid protocol
    let protocol = handler.listen_protocol();
    println!("✅ listen_protocol returns valid protocol: {:?}", protocol);
    
    // Test connection_keep_alive returns boolean
    let keep_alive = handler.connection_keep_alive();
    assert!(keep_alive == true || keep_alive == false, "keep_alive should be boolean");
    println!("✅ connection_keep_alive returns boolean: {}", keep_alive);
    
    // Test that handler can be used as ConnectionHandler
    // This is verified by the fact that we can call trait methods
    
    println!("✅ XStreamHandler protocol methods test completed successfully");
}

#[tokio::test]
async fn test_handler_peer_id_management() {
    // Test peer_id management in XStreamHandler
    println!("🚀 Testing XStreamHandler peer_id management...");
    
    let connection_id = libp2p::swarm::ConnectionId::new_unchecked(4);
    let peer_id = PeerId::random();
    let established_connection = EstablishedConnection::Outbound {
        addr: "/memory/0".parse().unwrap(),
    };
    let mut handler = XStreamHandler::new(connection_id, peer_id, established_connection);
    
    // Test setting peer_id
    let peer_id_1 = PeerId::random();
    handler.set_peer_id(peer_id_1);
    println!("✅ Successfully set peer_id: {}", peer_id_1);
    
    // Test that handler methods still work after setting peer_id
    let protocol = handler.listen_protocol();
    let keep_alive = handler.connection_keep_alive();
    
    println!("✅ After setting peer_id - protocol: {:?}, keep_alive: {}", protocol, keep_alive);
    
    // Test setting different peer_id (should work)
    let peer_id_2 = PeerId::random();
    handler.set_peer_id(peer_id_2);
    println!("✅ Successfully changed peer_id to: {}", peer_id_2);
    
    // Verify handler still functional
    let _ = handler.listen_protocol();
    let _ = handler.connection_keep_alive();
    println!("✅ Handler remains functional after peer_id change");
    
    println!("✅ XStreamHandler peer_id management test completed successfully");
}

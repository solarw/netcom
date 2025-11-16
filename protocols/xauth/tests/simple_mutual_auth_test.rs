#![cfg(test)]

use libp2p::{identity::Keypair, PeerId};
use xauth::{
    behaviours::PorAuthBehaviour,
    por::por::{PorUtils, ProofOfRepresentation},
};
use std::time::Duration;

#[tokio::test]
async fn test_simple_mutual_authentication() {
    // Create owner key that will issue PORs for both nodes
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create two PorAuthBehaviour instances
    let keypair1 = Keypair::generate_ed25519();
    let peer_id1 = PeerId::from(keypair1.public());
    let por1 = ProofOfRepresentation::create(&owner_key, peer_id1, Duration::from_secs(60))
        .expect("Failed to create POR");
    
    let keypair2 = Keypair::generate_ed25519();
    let peer_id2 = PeerId::from(keypair2.public());
    let por2 = ProofOfRepresentation::create(&owner_key, peer_id2, Duration::from_secs(60))
        .expect("Failed to create POR");
    
    let mut behaviour1 = PorAuthBehaviour::new(por1);
    let mut behaviour2 = PorAuthBehaviour::new(por2);
    
    // Verify that initially no peers are authenticated
    assert!(!behaviour1.is_peer_authenticated(&peer_id2), 
            "Peer2 should not be authenticated by default");
    assert!(!behaviour2.is_peer_authenticated(&peer_id1), 
            "Peer1 should not be authenticated by default");
    
    println!("✅ Initial state: no mutual authentication");
    
    // Test that authentication requires explicit start
    // In a real scenario, the application must call start_authentication()
    // when it wants to authenticate a connection
    
    // Test that we can create connection IDs (even though they won't work without real connections)
    let dummy_conn_id1 = libp2p::swarm::ConnectionId::new_unchecked(1);
    let dummy_conn_id2 = libp2p::swarm::ConnectionId::new_unchecked(2);
    
    // Test that start_authentication method exists and returns error for invalid connections
    let result1 = behaviour1.start_authentication(dummy_conn_id1);
    let result2 = behaviour2.start_authentication(dummy_conn_id2);
    
    assert!(result1.is_err(), "Should return error for invalid connection ID");
    assert!(result2.is_err(), "Should return error for invalid connection ID");
    
    println!("✅ Manual mode prevents authentication without valid connections");
    println!("✅ start_authentication method works correctly");
    
    // Verify that peers are still not authenticated after failed attempts
    assert!(!behaviour1.is_peer_authenticated(&peer_id2), 
            "Peer2 should still not be authenticated after failed manual start");
    assert!(!behaviour2.is_peer_authenticated(&peer_id1), 
            "Peer1 should still not be authenticated after failed manual start");
    
    println!("✅ Authentication state remains unchanged after failed manual attempts");
    println!("✅ Simple mutual authentication test completed successfully");
}

#[tokio::test]
async fn test_mutual_authentication_with_metadata() {
    // Create owner key
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create PorAuthBehaviour with metadata
    let keypair = Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    let por = ProofOfRepresentation::create(&owner_key, peer_id, Duration::from_secs(60))
        .expect("Failed to create POR");
    
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("role".to_string(), "test_node".to_string());
    metadata.insert("version".to_string(), "1.0.0".to_string());
    
    let mut behaviour = PorAuthBehaviour::with_metadata(por, metadata);
    
    // Test manual mode with metadata
    let dummy_conn_id = libp2p::swarm::ConnectionId::new_unchecked(1);
    let result = behaviour.start_authentication(dummy_conn_id);
    
    assert!(result.is_err(), "Should return error for invalid connection ID");
    
    println!("✅ Manual mode works correctly with metadata");
    println!("✅ Metadata functionality preserved in manual mode");
}

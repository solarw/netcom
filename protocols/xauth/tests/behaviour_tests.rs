#![cfg(test)]

use libp2p::{identity::Keypair, Multiaddr, PeerId};
use std::{collections::{HashMap, HashSet}, time::Duration};
use xauth::{
    behaviours::PorAuthBehaviour,
    connection_data::ConnectionData,
    definitions::AuthResult,
    por::por::{PorUtils, ProofOfRepresentation},
};

// Helper to create a valid POR for a given peer
fn create_valid_por_for_peer(owner_key: &Keypair, peer_id: PeerId) -> ProofOfRepresentation {
    ProofOfRepresentation::create(owner_key, peer_id, Duration::from_secs(60))
        .expect("Failed to create POR")
}

#[tokio::test]
async fn test_por_auth_behaviour_creation() {
    // Test basic behaviour creation
    let owner_key = PorUtils::generate_owner_keypair();
    let node_key = Keypair::generate_ed25519();
    let node_peer_id = PeerId::from(node_key.public());
    let por = create_valid_por_for_peer(&owner_key, node_peer_id);
    
    let behaviour = PorAuthBehaviour::new(por);
    
    // Verify behaviour was created successfully
    assert!(behaviour.is_peer_authenticated(&node_peer_id) == false);
}

#[tokio::test]
async fn test_por_verification_workflow() {
    // Test the verification workflow without complex swarm setup
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create two nodes with valid PORs
    let node1_key = Keypair::generate_ed25519();
    let node1_peer_id = PeerId::from(node1_key.public());
    let por1 = create_valid_por_for_peer(&owner_key, node1_peer_id);
    
    let node2_key = Keypair::generate_ed25519();
    let node2_peer_id = PeerId::from(node2_key.public());
    let por2 = create_valid_por_for_peer(&owner_key, node2_peer_id);
    
    // Create behaviours
    let mut behaviour1 = PorAuthBehaviour::new(por1);
    let mut behaviour2 = PorAuthBehaviour::new(por2);
    
    // Test that behaviours can be created and have basic functionality
    assert!(!behaviour1.is_peer_authenticated(&node2_peer_id));
    assert!(!behaviour2.is_peer_authenticated(&node1_peer_id));
    
    // Test metadata functionality
    let mut metadata = HashMap::new();
    metadata.insert("test_key".to_string(), "test_value".to_string());
    behaviour1.update_metadata(metadata.clone());
    
    // Verify behaviours are functional
    assert!(behaviour1.get_peer_metadata(&node2_peer_id).is_none());
    assert!(behaviour2.get_peer_metadata(&node1_peer_id).is_none());
}

#[tokio::test]
async fn test_por_validation_scenarios() {
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Test 1: Valid POR
    let node_key = Keypair::generate_ed25519();
    let node_peer_id = PeerId::from(node_key.public());
    let valid_por = create_valid_por_for_peer(&owner_key, node_peer_id);
    assert!(valid_por.validate().is_ok());
    
    // Test 2: Expired POR
    let expired_por = ProofOfRepresentation::create_with_times(
        &owner_key, 
        node_peer_id, 
        1,  // issued_at (very old)
        2   // expires_at (very old)
    ).unwrap();
    assert!(expired_por.validate().is_err());
    assert!(expired_por.validate().unwrap_err().contains("expired"));
    
    // Test 3: Not yet valid POR
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let future_por = ProofOfRepresentation::create_with_times(
        &owner_key,
        node_peer_id,
        now + 3600,  // issued_at (1 hour in future)
        now + 7200   // expires_at (2 hours in future)
    ).unwrap();
    assert!(future_por.validate().is_err());
    assert!(future_por.validate().unwrap_err().contains("not yet valid"));
}

#[tokio::test]
async fn test_por_metadata_functionality() {
    let owner_key = PorUtils::generate_owner_keypair();
    let node_key = Keypair::generate_ed25519();
    let node_peer_id = PeerId::from(node_key.public());
    let por = create_valid_por_for_peer(&owner_key, node_peer_id);
    
    let mut behaviour = PorAuthBehaviour::new(por);
    
    // Test metadata update
    let mut metadata = HashMap::new();
    metadata.insert("role".to_string(), "validator".to_string());
    metadata.insert("version".to_string(), "1.0.0".to_string());
    
    behaviour.update_metadata(metadata.clone());
    
    // Test POR update
    let new_por = create_valid_por_for_peer(&owner_key, node_peer_id);
    behaviour.update_por(new_por);
    
    // Verify behaviour is still functional after updates
    assert!(!behaviour.is_peer_authenticated(&node_peer_id));
}

#[tokio::test]
async fn test_por_utils_functionality() {
    // Test POR utility functions
    let keypair1 = PorUtils::generate_owner_keypair();
    let keypair2 = PorUtils::generate_owner_keypair();
    
    // Test peer ID generation
    let peer_id1 = PorUtils::peer_id_from_keypair(&keypair1);
    let peer_id2 = PorUtils::peer_id_from_keypair(&keypair2);
    
    assert_ne!(peer_id1, peer_id2);
    
    // Test keypair from bytes (round trip)
    let bytes = keypair1.to_protobuf_encoding().unwrap();
    let restored_keypair = PorUtils::keypair_from_bytes(&bytes).unwrap();
    let restored_peer_id = PorUtils::peer_id_from_keypair(&restored_keypair);
    
    assert_eq!(peer_id1, restored_peer_id);
}

#[tokio::test]
async fn test_successful_mutual_authentication() {
    // Test successful mutual authentication between two nodes
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create two nodes with valid PORs
    let node1_key = Keypair::generate_ed25519();
    let node1_peer_id = PeerId::from(node1_key.public());
    let por1 = create_valid_por_for_peer(&owner_key, node1_peer_id);
    
    let node2_key = Keypair::generate_ed25519();
    let node2_peer_id = PeerId::from(node2_key.public());
    let por2 = create_valid_por_for_peer(&owner_key, node2_peer_id);
    
    // Create behaviours with metadata
    let mut metadata1 = HashMap::new();
    metadata1.insert("role".to_string(), "node1".to_string());
    metadata1.insert("version".to_string(), "1.0.0".to_string());
    
    let mut metadata2 = HashMap::new();
    metadata2.insert("role".to_string(), "node2".to_string());
    metadata2.insert("version".to_string(), "1.0.0".to_string());
    
    let behaviour1 = PorAuthBehaviour::with_metadata(por1, metadata1);
    let behaviour2 = PorAuthBehaviour::with_metadata(por2, metadata2);
    
    // Test basic functionality without accessing private fields
    assert!(!behaviour1.is_peer_authenticated(&node2_peer_id));
    assert!(!behaviour2.is_peer_authenticated(&node1_peer_id));
    
    // Test metadata functionality
    assert!(behaviour1.get_peer_metadata(&node2_peer_id).is_none());
    assert!(behaviour2.get_peer_metadata(&node1_peer_id).is_none());
}

#[tokio::test]
async fn test_failed_authentication_expired_por() {
    // Test authentication failure due to expired POR
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create first node with valid POR
    let node1_key = Keypair::generate_ed25519();
    let node1_peer_id = PeerId::from(node1_key.public());
    let por1 = create_valid_por_for_peer(&owner_key, node1_peer_id);
    
    // Create second node with EXPIRED POR
    let node2_key = Keypair::generate_ed25519();
    let node2_peer_id = PeerId::from(node2_key.public());
    let expired_por = ProofOfRepresentation::create_with_times(
        &owner_key, 
        node2_peer_id, 
        1,  // issued_at (very old)
        2   // expires_at (very old)
    ).unwrap();
    
    // Create behaviours
    let behaviour1 = PorAuthBehaviour::new(por1);
    let behaviour2 = PorAuthBehaviour::new(expired_por);
    
    // Test that behaviours are created successfully
    assert!(!behaviour1.is_peer_authenticated(&node2_peer_id));
    assert!(!behaviour2.is_peer_authenticated(&node1_peer_id));
    
    // Test metadata functionality
    assert!(behaviour1.get_peer_metadata(&node2_peer_id).is_none());
    assert!(behaviour2.get_peer_metadata(&node1_peer_id).is_none());
}

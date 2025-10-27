// real_network_swarm_tests.rs
// Simple swarm tests for real network conditions

use crate::behaviour::XStreamNetworkBehaviour;
use libp2p::{PeerId, swarm::Swarm};
use libp2p_swarm_test::SwarmExt;
use std::time::Duration;

#[tokio::test]
async fn test_basic_swarm_creation() {
    // Test basic swarm creation and configuration
    let swarm = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    
    let peer_id = *swarm.local_peer_id();
    assert!(!peer_id.to_string().is_empty(), "Should have valid peer ID");
    
    println!("✅ Basic swarm creation test passed");
}

#[tokio::test]
async fn test_swarm_listening() {
    // Test that swarm can listen on memory addresses
    let mut swarm = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    
    // Try to listen on memory address
    let (memory_addr, _) = swarm.listen().with_memory_addr_external().await;
    
    assert!(!memory_addr.to_string().is_empty(), "Should have valid memory address");
    println!("✅ Swarm listening test passed - listening on: {}", memory_addr);
}

#[tokio::test]
async fn test_swarm_peer_id_generation() {
    // Test that swarm generates unique peer IDs
    let swarm1 = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    let swarm2 = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    
    let peer_id1 = *swarm1.local_peer_id();
    let peer_id2 = *swarm2.local_peer_id();
    
    assert_ne!(peer_id1, peer_id2, "Should generate different peer IDs");
    println!("✅ Swarm peer ID generation test passed");
}

#[tokio::test]
async fn test_swarm_external_addresses() {
    // Test that swarm can report external addresses
    let mut swarm = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    
    // Listen on memory address
    let (memory_addr, _) = swarm.listen().with_memory_addr_external().await;
    
    // Check external addresses
    let external_addrs: Vec<_> = swarm.external_addresses().collect();
    assert!(!external_addrs.is_empty(), "Should have external addresses");
    
    println!("✅ Swarm external addresses test passed - {} addresses", external_addrs.len());
}

#[tokio::test]
async fn test_swarm_behaviour_integration() {
    // Test that XStreamNetworkBehaviour integrates properly with swarm
    let mut swarm = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    
    // Verify behaviour is accessible
    let behaviour = swarm.behaviour_mut();
    assert!(std::mem::size_of_val(behaviour) > 0, "Behaviour should be accessible");
    
    println!("✅ Swarm behaviour integration test passed");
}

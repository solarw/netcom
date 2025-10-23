#![cfg(test)]

use libp2p::{identity::Keypair, PeerId};
use libp2p_swarm::{Swarm, SwarmEvent};
use libp2p_swarm_test::SwarmExt;
use std::{collections::HashMap, time::{Duration, Instant}};
use xauth::{
    behaviours::PorAuthBehaviour,
    events::PorAuthEvent,
    por::por::{PorUtils, ProofOfRepresentation},
};

// Helper to create a valid POR for a given peer
fn create_valid_por_for_peer(owner_key: &Keypair, peer_id: PeerId) -> ProofOfRepresentation {
    ProofOfRepresentation::create(owner_key, peer_id, Duration::from_secs(60))
        .expect("Failed to create POR")
}

// Helper to wait for mutual authentication between two nodes
async fn wait_for_mutual_authentication(
    swarm1: &mut Swarm<PorAuthBehaviour>,
    swarm2: &mut Swarm<PorAuthBehaviour>,
    timeout: Duration
) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        // Check if both nodes have authenticated each other
        let swarm1_authenticated = swarm1.behaviour().is_peer_authenticated(swarm2.local_peer_id());
        let swarm2_authenticated = swarm2.behaviour().is_peer_authenticated(swarm1.local_peer_id());
        
        if swarm1_authenticated && swarm2_authenticated {
            println!("Mutual authentication achieved!");
            return true;
        }
        
        // Process events from both swarms to allow authentication to progress
        // Process swarm1 events
        if let Ok(event) = tokio::time::timeout(Duration::from_millis(50), swarm1.next_swarm_event()).await {
            handle_swarm_event(swarm1, event).await;
        }
        
        // Process swarm2 events
        if let Ok(event) = tokio::time::timeout(Duration::from_millis(50), swarm2.next_swarm_event()).await {
            handle_swarm_event(swarm2, event).await;
        }
        
        // Small delay to prevent busy waiting
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    println!("Authentication timeout reached after {:?}", start.elapsed());
    println!("Swarm1 authenticated Swarm2: {}", swarm1.behaviour().is_peer_authenticated(swarm2.local_peer_id()));
    println!("Swarm2 authenticated Swarm1: {}", swarm2.behaviour().is_peer_authenticated(swarm1.local_peer_id()));
    false
}

// Helper to handle swarm events, particularly PorAuthEvent::VerifyPorRequest
async fn handle_swarm_event(swarm: &mut Swarm<PorAuthBehaviour>, event: libp2p_swarm::SwarmEvent<PorAuthEvent>) {
    match event {
        libp2p_swarm::SwarmEvent::Behaviour(PorAuthEvent::VerifyPorRequest {
            peer_id,
            connection_id,
            por,
            metadata,
            ..
        }) => {
            println!("Processing VerifyPorRequest from peer {:?} on connection {:?}", peer_id, connection_id);
            
            // Auto-approve the POR if it's valid
            let result = if por.validate().is_ok() {
                println!("Auto-approving valid POR from peer {:?}", peer_id);
                xauth::definitions::AuthResult::Ok(metadata)
            } else {
                println!("Auto-rejecting invalid POR from peer {:?}", peer_id);
                xauth::definitions::AuthResult::Error("POR validation failed".to_string())
            };
            
            // Submit the verification result
            if let Err(e) = swarm.behaviour_mut().submit_por_verification_result(connection_id, result) {
                println!("Failed to submit POR verification result: {}", e);
            }
        }
        _ => {
            // Ignore other events for now
        }
    }
}

#[tokio::test]
async fn por_auth_mutual_authentication() {
    // Create owner key that will issue PORs for both nodes
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create two Swarm instances with PorAuthBehaviour
    let mut swarm1 = Swarm::new_ephemeral_tokio(|keypair| {
        let peer_id = PeerId::from(keypair.public());
        let por = create_valid_por_for_peer(&owner_key, peer_id);
        
        // Add some metadata to test metadata functionality
        let mut metadata = HashMap::new();
        metadata.insert("role".to_string(), "node1".to_string());
        metadata.insert("version".to_string(), "1.0.0".to_string());
        
        PorAuthBehaviour::with_metadata(por, metadata)
    });
    
    let mut swarm2 = Swarm::new_ephemeral_tokio(|keypair| {
        let peer_id = PeerId::from(keypair.public());
        let por = create_valid_por_for_peer(&owner_key, peer_id);
        
        // Add some metadata to test metadata functionality
        let mut metadata = HashMap::new();
        metadata.insert("role".to_string(), "node2".to_string());
        metadata.insert("version".to_string(), "1.0.0".to_string());
        
        PorAuthBehaviour::with_metadata(por, metadata)
    });

    // Start listening on memory transport and make addresses external
    let (memory_addr1, _tcp_addr1) = swarm1.listen().with_memory_addr_external().await;
    let (memory_addr2, _tcp_addr2) = swarm2.listen().with_memory_addr_external().await;
    
    println!("Swarm1 listening on: {}", memory_addr1);
    println!("Swarm2 listening on: {}", memory_addr2);
    println!("Swarm1 peer ID: {}", swarm1.local_peer_id());
    println!("Swarm2 peer ID: {}", swarm2.local_peer_id());

    // Connect swarm2 to swarm1
    swarm2.connect(&mut swarm1).await;
    
    println!("Connection established between nodes");

    // Wait for mutual authentication with timeout
    let authenticated = wait_for_mutual_authentication(
        &mut swarm1, 
        &mut swarm2, 
        Duration::from_secs(5)
    ).await;
    
    // Verify that mutual authentication occurred
    assert!(authenticated, "Nodes should mutually authenticate within 5 seconds");
    
    // Additional verification using the public method
    let swarm1_peer_id = *swarm1.local_peer_id();
    let swarm2_peer_id = *swarm2.local_peer_id();
    
    assert!(
        swarm1.behaviour().is_peer_authenticated(&swarm2_peer_id),
        "Swarm1 should have authenticated Swarm2"
    );
    assert!(
        swarm2.behaviour().is_peer_authenticated(&swarm1_peer_id),
        "Swarm2 should have authenticated Swarm1"
    );
    
    println!("Test completed successfully - mutual authentication verified");
    println!("Swarm1 peer ID: {}", swarm1_peer_id);
    println!("Swarm2 peer ID: {}", swarm2_peer_id);
}

#[tokio::test]
async fn por_auth_failed_authentication_expired_por() {
    // Create owner key
    let owner_key = PorUtils::generate_owner_keypair();
    
    // Create first node with valid POR
    let mut swarm1 = Swarm::new_ephemeral_tokio(|keypair| {
        let peer_id = PeerId::from(keypair.public());
        let por = create_valid_por_for_peer(&owner_key, peer_id);
        PorAuthBehaviour::new(por)
    });
    
    // Create second node with EXPIRED POR
    let mut swarm2 = Swarm::new_ephemeral_tokio(|keypair| {
        let peer_id = PeerId::from(keypair.public());
        // Create expired POR (issued and expired in the past)
        let expired_por = ProofOfRepresentation::create_with_times(
            &owner_key, 
            peer_id, 
            1,  // issued_at (very old)
            2   // expires_at (very old)
        ).unwrap();
        PorAuthBehaviour::new(expired_por)
    });

    // Start listening
    swarm1.listen().with_memory_addr_external().await;
    swarm2.listen().with_memory_addr_external().await;
    
    println!("Testing expired POR scenario");
    println!("Swarm1 (valid POR) peer ID: {}", swarm1.local_peer_id());
    println!("Swarm2 (expired POR) peer ID: {}", swarm2.local_peer_id());

    // Connect swarm2 to swarm1
    swarm2.connect(&mut swarm1).await;
    
    println!("Connection established - testing authentication behavior with expired POR");

    // Wait for potential authentication (should fail with expired POR)
    let authenticated = wait_for_mutual_authentication(
        &mut swarm1, 
        &mut swarm2, 
        Duration::from_secs(2) // Short timeout since authentication should fail
    ).await;
    
    // Verify that authentication failed as expected
    assert!(!authenticated, "Authentication should fail with expired POR");
    
    // Additional verification using the public method
    let swarm1_peer_id = *swarm1.local_peer_id();
    let swarm2_peer_id = *swarm2.local_peer_id();
    
    assert!(
        !swarm1.behaviour().is_peer_authenticated(&swarm2_peer_id),
        "Swarm1 should NOT have authenticated Swarm2 (expired POR)"
    );
    assert!(
        !swarm2.behaviour().is_peer_authenticated(&swarm1_peer_id),
        "Swarm2 should NOT have authenticated Swarm1 (expired POR)"
    );
    
    println!("Test completed - authentication correctly failed with expired POR");
}

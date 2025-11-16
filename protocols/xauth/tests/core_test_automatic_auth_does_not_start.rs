#![cfg(test)]

use libp2p::{identity, quic, Multiaddr, PeerId};
use libp2p::futures::StreamExt;
use libp2p_swarm::{Swarm, SwarmEvent};
use std::{collections::HashMap, time::{Duration, Instant}};
use tokio::time::timeout;
use xauth::{
    behaviours::PorAuthBehaviour,
    events::PorAuthEvent,
    por::por::{PorUtils, ProofOfRepresentation},
};

// Helper to create a valid POR for a given peer
fn create_valid_por_for_peer(owner_key: &identity::Keypair, peer_id: PeerId) -> ProofOfRepresentation {
    ProofOfRepresentation::create(owner_key, peer_id, Duration::from_secs(60))
        .expect("Failed to create POR")
}

/// Creates a real QUIC swarm with PorAuthBehaviour
async fn create_quic_swarm_with_auth() -> (Swarm<PorAuthBehaviour>, PeerId) {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();
    
    // Create owner key that will issue PORs
    let owner_key = PorUtils::generate_owner_keypair();
    let por = create_valid_por_for_peer(&owner_key, peer_id);
    
    // Add some metadata to test metadata functionality
    let mut metadata = HashMap::new();
    metadata.insert("role".to_string(), "test_node".to_string());
    metadata.insert("version".to_string(), "1.0.0".to_string());
    
    // Create QUIC transport
    let quic_config = quic::Config::new(&keypair);
    let quic_transport = quic::tokio::Transport::new(quic_config);
    
    // Create swarm with PorAuthBehaviour
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|_key| quic_transport)
        .expect("Failed to create QUIC transport")
        .with_behaviour(|_key| PorAuthBehaviour::with_metadata(por, metadata))
        .expect("Failed to create PorAuthBehaviour")
        .build();
    
    (swarm, peer_id)
}

/// Waits for a listen address from swarm
async fn wait_for_listen_addr(swarm: &mut Swarm<PorAuthBehaviour>) -> Multiaddr {
    timeout(Duration::from_secs(2), async {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                return address;
            }
        }
    })
    .await
    .expect("Timeout waiting for listen address")
}

#[tokio::test]
async fn test_automatic_auth_does_not_start_with_quic() {
    println!("🚀 Starting QUIC automatic auth prevention test...");

    // Create two real QUIC swarm instances with PorAuthBehaviour
    let (mut swarm1, swarm1_peer_id) = create_quic_swarm_with_auth().await;
    let (mut swarm2, swarm2_peer_id) = create_quic_swarm_with_auth().await;

    println!("✅ Created two QUIC swarm nodes:");
    println!("   Swarm1 peer ID: {}", swarm1_peer_id);
    println!("   Swarm2 peer ID: {}", swarm2_peer_id);

    // Start listening on QUIC transport
    let server_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().expect("Invalid server address");
    swarm1.listen_on(server_addr.clone()).expect("Failed to listen");
    
    // Get actual listen address
    let listen_addr = wait_for_listen_addr(&mut swarm1).await;
    println!("✅ Swarm1 listening on: {}", listen_addr);

    // Phase 1: Establish real QUIC connection WITHOUT starting authentication
    let mut connection_ids: Vec<(u32, libp2p_swarm::ConnectionId, PeerId)> = Vec::new();
    let mut mutual_auth_events: Vec<(u32, PeerId, HashMap<String, String>)> = Vec::new();
    let mut manual_mode_messages: Vec<String> = Vec::new();
    
    println!("🔄 Establishing QUIC connection WITHOUT starting authentication...");
    
    // Connect swarm2 to swarm1
    swarm2.dial(listen_addr.clone()).expect("Failed to dial");
    
    let connection_timeout = Duration::from_secs(10);
    let start = Instant::now();
    
    while start.elapsed() < connection_timeout && connection_ids.len() < 2 {
        // Process swarm1 events
        if let Ok(event) = timeout(Duration::from_millis(100), swarm1.select_next_some()).await {
            match event {
                SwarmEvent::ConnectionEstablished { connection_id, peer_id, .. } => {
                    println!("✅ Swarm1: ConnectionEstablished with peer {} on connection {:?}", peer_id, connection_id);
                    connection_ids.push((1, connection_id, peer_id));
                    
                    // IMPORTANT: DO NOT call start_authentication() - this is the key test!
                    println!("🔄 Swarm1: NOT starting manual authentication for connection {:?} (this is intentional)", connection_id);
                }
                SwarmEvent::Behaviour(PorAuthEvent::VerifyPorRequest { .. }) => {
                    panic!("❌ Swarm1: Should NOT receive VerifyPorRequest without manual authentication start!");
                }
                SwarmEvent::Behaviour(PorAuthEvent::MutualAuthSuccess { .. }) => {
                    panic!("❌ Swarm1: Should NOT receive MutualAuthSuccess without manual authentication start!");
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("📡 Swarm1: NewListenAddr: {}", address);
                }
                _ => {}
            }
        }
        
        // Process swarm2 events
        if let Ok(event) = timeout(Duration::from_millis(100), swarm2.select_next_some()).await {
            match event {
                SwarmEvent::ConnectionEstablished { connection_id, peer_id, .. } => {
                    println!("✅ Swarm2: ConnectionEstablished with peer {} on connection {:?}", peer_id, connection_id);
                    connection_ids.push((2, connection_id, peer_id));
                    
                    // IMPORTANT: DO NOT call start_authentication() - this is the key test!
                    println!("🔄 Swarm2: NOT starting manual authentication for connection {:?} (this is intentional)", connection_id);
                }
                SwarmEvent::Behaviour(PorAuthEvent::VerifyPorRequest { .. }) => {
                    panic!("❌ Swarm2: Should NOT receive VerifyPorRequest without manual authentication start!");
                }
                SwarmEvent::Behaviour(PorAuthEvent::MutualAuthSuccess { .. }) => {
                    panic!("❌ Swarm2: Should NOT receive MutualAuthSuccess without manual authentication start!");
                }
                _ => {}
            }
        }
        
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Verify that we got connection IDs (connection is established)
    assert!(
        connection_ids.len() >= 2,
        "Should have at least 2 connection IDs (one for each direction), but got {}",
        connection_ids.len()
    );
    println!("✅ Both connection directions established");

    // Verify that we got NO mutual authentication events
    assert_eq!(
        mutual_auth_events.len(), 
        0, 
        "Should have 0 MutualAuthSuccess events without manual authentication, but got {}",
        mutual_auth_events.len()
    );
    println!("✅ No mutual authentication events received (as expected)");

    // Verify that both nodes are NOT authenticated
    assert!(
        !swarm1.behaviour().is_peer_authenticated(&swarm2_peer_id),
        "Swarm1 should NOT have authenticated Swarm2 without manual authentication"
    );
    assert!(
        !swarm2.behaviour().is_peer_authenticated(&swarm1_peer_id),
        "Swarm2 should NOT have authenticated Swarm1 without manual authentication"
    );
    println!("✅ Both nodes are NOT mutually authenticated (as expected)");

    // Verify metadata functionality - should NOT have metadata
    let metadata1 = swarm1.behaviour().get_peer_metadata(&swarm2_peer_id);
    let metadata2 = swarm2.behaviour().get_peer_metadata(&swarm1_peer_id);
    
    assert!(
        metadata1.is_none(), 
        "Swarm1 should NOT have metadata for Swarm2 without authentication"
    );
    assert!(
        metadata2.is_none(), 
        "Swarm2 should NOT have metadata for Swarm1 without authentication"
    );
    
    println!("✅ Metadata NOT available for unauthenticated peers (as expected)");

    println!("🎉 Test completed successfully - manual mode prevents automatic authentication!");
    println!("✅ Real QUIC connections established");
    println!("✅ Manual authentication NOT started (intentionally)");
    println!("✅ NO MutualAuthSuccess events received");
    println!("✅ Both nodes NOT mutually authenticated");
    println!("✅ Metadata NOT available");
    println!("✅ Manual mode works correctly - prevents automatic authentication!");
}

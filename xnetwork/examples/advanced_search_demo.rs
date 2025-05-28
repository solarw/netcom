// examples/advanced_search_demo.rs

use std::time::Duration;
use libp2p::{identity, PeerId};
use xnetwork::{NetworkNode, XRoutesConfig, XRouteRole};

#[tokio::main]
async fn main() -> Result<(), String> {
    // Initialize simple logging
    println!("🚀 Starting Advanced Search Demo");

    // Create test PoR
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    let por = xauth::por::por::ProofOfRepresentation::create(
        &keypair,
        peer_id,
        Duration::from_secs(3600),
    ).map_err(|e| format!("Failed to create PoR: {}", e))?;

    // Create client node with DHT enabled
    let config = XRoutesConfig {
        enable_mdns: true,
        enable_kad: true,
        kad_server_mode: false,
        initial_role: XRouteRole::Client,
        enable_relay_client: false,
        enable_relay_server: false,
        known_relay_servers: Vec::new(),
    };

    let key = identity::Keypair::generate_ed25519();
    let (mut node, cmd_tx, mut _event_rx, local_peer_id) = 
        NetworkNode::new_with_config(key, por, Some(config)).await
        .map_err(|e| format!("Failed to create node: {}", e))?;

    let commander = std::sync::Arc::new(xnetwork::Commander::new(cmd_tx));

    println!("📍 Local Peer ID: {}", local_peer_id);

    // Start the node
    tokio::spawn(async move {
        node.run().await;
    });

    // Give node time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Demo different search scenarios
    let demo_peer1 = PeerId::random();
    let demo_peer2 = PeerId::random();
    let demo_peer3 = PeerId::random();

    println!("🔍 Demo 1: Local-only searches");
    for (i, target) in [demo_peer1, demo_peer2, demo_peer3].iter().enumerate() {
        println!("  Searching for peer {} locally...", i + 1);
        match commander.find_peer_addresses_advanced(*target, 0).await {
            Ok(addresses) => {
                println!("    ✅ Found {} addresses", addresses.len());
            }
            Err(e) => {
                println!("    ❌ Search failed: {}", e);
            }
        }
    }

    println!("🔍 Demo 2: Concurrent searches with different timeouts");
    let target = demo_peer1;
    
    let search1 = commander.find_peer_addresses_advanced(target, 2);
    let search2 = commander.find_peer_addresses_advanced(target, 5);
    let search3 = commander.find_peer_addresses_advanced(target, 10);

    println!("  Starting 3 concurrent searches for same peer...");
    let start = std::time::Instant::now();
    
    let (result1, result2, result3) = tokio::join!(search1, search2, search3);
    
    let elapsed = start.elapsed();
    println!("  All searches completed in {:?}", elapsed);
    println!("    2s search: {:?}", result1.map(|a| a.len()));
    println!("    5s search: {:?}", result2.map(|a| a.len()));
    println!("    10s search: {:?}", result3.map(|a| a.len()));

    println!("🔍 Demo 3: Search cancellation");
    let target = demo_peer2;
    
    // Start long search
    let search_future = commander.find_peer_addresses_advanced(target, 30);
    println!("  Started 30-second search...");
    
    // Wait 2 seconds then cancel
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("  Cancelling search...");
    
    match commander.cancel_peer_search(target).await {
        Ok(_) => println!("    ✅ Cancellation requested"),
        Err(e) => println!("    ❌ Cancellation failed: {}", e),
    }

    // Wait for search to complete
    match tokio::time::timeout(Duration::from_secs(5), search_future).await {
        Ok(result) => println!("    Search result: {:?}", result.map(|a| a.len())),
        Err(_) => println!("    Search timed out in demo"),
    }

    println!("🔍 Demo 4: Monitoring active searches");
    
    // Start several searches with Arc clones
    let commander1 = commander.clone();
    let commander2 = commander.clone();
    let commander3 = commander.clone();
    
    let _search1 = tokio::spawn(async move {
        commander1.find_peer_addresses_advanced(demo_peer1, 15).await
    });
    let _search2 = tokio::spawn(async move {
        commander2.find_peer_addresses_advanced(demo_peer2, 20).await
    });
    let _search3 = tokio::spawn(async move {
        commander3.find_peer_addresses_advanced(demo_peer1, 25).await // Same peer as search1
    });
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    match commander.get_active_searches().await {
        Ok(searches) => {
            println!("  Active searches: {}", searches.len());
            for (peer_id, waiters, duration) in searches {
                println!("    Peer {}: {} waiters, running for {:?}", peer_id, waiters, duration);
            }
        }
        Err(e) => println!("  Failed to get active searches: {}", e),
    }

    println!("🔍 Demo 5: Convenience methods");
    let target = demo_peer3;
    
    // Test convenience methods
    println!("  Testing convenience methods...");
    
    let local_result = commander.find_peer_addresses_local_only(target).await;
    println!("    Local only: {:?}", local_result.map(|a| a.len()));
    
    let timeout_result = commander.find_peer_addresses_with_timeout(target, 3).await;
    println!("    With 3s timeout: {:?}", timeout_result.map(|a| a.len()));

    // Wait a bit to see periodic cleanup in action
    println!("⏳ Waiting to observe periodic cleanup...");
    tokio::time::sleep(Duration::from_secs(35)).await;

    println!("✅ Demo completed!");
    Ok(())
}

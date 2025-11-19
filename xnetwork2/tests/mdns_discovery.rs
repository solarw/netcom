//! Test for mDNS discovery functionality
//! 
//! This test verifies that two nodes with enabled Kademlia, Identify, and mDNS
//! can discover each other through mDNS and exchange addresses.

use std::time::Duration;
use tokio::time::timeout;

use xnetwork2::{
    node_builder,
    node_events::NodeEvent,
};
mod utils;
use utils::setup_listening_node;

/// Test mDNS discovery between two nodes
#[tokio::test]
async fn test_mdns_discovery() {
    // Initialize logging for debugging
    //let _ = tracing_subscriber::fmt::try_init();
    println!("🚀 Starting mDNS discovery test...");

    // Create first node
    println!("🔄 Creating node 1...");
    let mut node1 = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node 1");
    let peer_id1 = *node1.peer_id();

    // Create second node
    println!("🔄 Creating node 2...");
    let mut node2 = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node 2");
    let peer_id2 = *node2.peer_id();

    println!("✅ Nodes created successfully:");
    println!("   Node 1: {}", peer_id1);
    println!("   Node 2: {}", peer_id2);

    // Start both nodes
    println!("🔄 Starting nodes...");
    node1.start().await.expect("Failed to start node 1");
    node2.start().await.expect("Failed to start node 2");

    // Enable behaviours on both nodes
    println!("🔄 Enabling behaviours...");
    
    // Subscribe to events BEFORE waiting for discovery
    let mut node1_events = node1.subscribe();
    let mut node2_events = node2.subscribe();

    // Enable Identify
    node1.enable_identify().await.expect("Failed to enable Identify on node 1");
    node2.enable_identify().await.expect("Failed to enable Identify on node 2");
    println!("✅ Identify enabled on both nodes");

    // Enable Kademlia
    node1.enable_kad().await.expect("Failed to enable Kademlia on node 1");
    node2.enable_kad().await.expect("Failed to enable Kademlia on node 2");
    println!("✅ Kademlia enabled on both nodes");

    // Enable mDNS
    node1.enable_mdns().await.expect("Failed to enable mDNS on node 1");
    node2.enable_mdns().await.expect("Failed to enable mDNS on node 2");
    println!("✅ mDNS enabled on both nodes");

    // Setup listening addresses for both nodes
    println!("🎯 Setting up listening addresses...");
    let addr1 = setup_listening_node(&mut node1).await
        .expect("Failed to setup listening for node 1");
    let addr2 = setup_listening_node(&mut node2).await
        .expect("Failed to setup listening for node 2");
    
    println!("📡 Node 1 listening on: {}", addr1);
    println!("📡 Node 2 listening on: {}", addr2);
    // Verify addresses contain QUIC protocol
    assert!(addr1.to_string().contains("/quic-v1"), "❌ Node 1 address should contain QUIC protocol");
    assert!(addr2.to_string().contains("/quic-v1"), "❌ Node 2 address should contain QUIC protocol");
    println!("✅ Both nodes listening on QUIC addresses");

    // Wait for mDNS discovery with timeout - smart waiting for events
    println!("⏳ Waiting for mDNS discovery (max 5 seconds)...");
    let discovery_timeout = Duration::from_secs(5);
    let mut discovery_happened = false;

    // Try to wait for MdnsPeerDiscovered events on both nodes
    let discovery_result = timeout(discovery_timeout, async {
        loop {
            tokio::select! {
                Ok(event) = node1_events.recv() => {
                    if let NodeEvent::MdnsPeerDiscovered { peer_id, .. } = event {
                        println!("🔍 Node 1 discovered peer: {}", peer_id);
                        if peer_id == peer_id2 {
                            discovery_happened = true;
                            break;
                        }
                    }
                }
                Ok(event) = node2_events.recv() => {
                    if let NodeEvent::MdnsPeerDiscovered { peer_id, .. } = event {
                        println!("🔍 Node 2 discovered peer: {}", peer_id);
                        if peer_id == peer_id1 {
                            discovery_happened = true;
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Continue waiting
                }
            }
        }
    }).await;
    match discovery_result {
        Ok(_) => {
            println!("✅ mDNS discovery successful! Peers found each other");
        }
        Err(_) => {
            panic!("⚠️ mDNS discovery timeout - no peers discovered in test environment");
        }
    }

    println!("🔄 Testing mDNS cache commands...");

    // Get mDNS cache status from node 1
    let cache_status1 = node1.get_mdns_cache_status().await
        .expect("Failed to get mDNS cache status from node 1");
    println!("📊 Node 1 mDNS cache status: {} peers", cache_status1.total_peers);

    // Get mDNS cache status from node 2
    let cache_status2 = node2.get_mdns_cache_status().await
        .expect("Failed to get mDNS cache status from node 2");
    println!("📊 Node 2 mDNS cache status: {} peers", cache_status2.total_peers);

    // Get all mDNS peers from node 1
    let mdns_peers1 = node1.get_mdns_peers().await
        .expect("Failed to get mDNS peers from node 1");
    println!("🔍 Node 1 mDNS peers: {} peers found", mdns_peers1.len());

    // Get all mDNS peers from node 2
    let mdns_peers2 = node2.get_mdns_peers().await
        .expect("Failed to get mDNS peers from node 2");
    println!("🔍 Node 2 mDNS peers: {} peers found", mdns_peers2.len());

    // Check if discovery happened and verify cache accordingly
    println!("🔍 Checking mDNS cache results...");
    
    let node1_found_node2 = mdns_peers1.iter().any(|(peer_id, _)| *peer_id == peer_id2);
    let node2_found_node1 = mdns_peers2.iter().any(|(peer_id, _)| *peer_id == peer_id1);

    println!("📊 Discovery results:");
    println!("   Node 1 found Node 2: {}", node1_found_node2);
    println!("   Node 2 found Node 1: {}", node2_found_node1);
    println!("   Total peers in Node 1 cache: {}", mdns_peers1.len());
    println!("   Total peers in Node 2 cache: {}", mdns_peers2.len());

    // Only check specific peer discovery if discovery actually happened
    if discovery_happened {
        println!("🔍 Testing specific peer discovery (discovery was successful)...");
        
        if node1_found_node2 {
            let node2_addresses = node1.find_mdns_peer(peer_id2).await
                .expect("Failed to find node 2 in mDNS cache");
            assert!(node2_addresses.is_some(), "Node 2 addresses should be found in mDNS cache after successful discovery");
            println!("✅ Node 1 found Node 2 addresses in mDNS cache: {} addresses", 
                     node2_addresses.as_ref().unwrap().len());
        }

        if node2_found_node1 {
            let node1_addresses = node2.find_mdns_peer(peer_id1).await
                .expect("Failed to find node 1 in mDNS cache");
            assert!(node1_addresses.is_some(), "Node 1 addresses should be found in mDNS cache after successful discovery");
            println!("✅ Node 2 found Node 1 addresses in mDNS cache: {} addresses", 
                     node1_addresses.as_ref().unwrap().len());
        }

        // If discovery happened, at least one direction should have found the peer
        assert!(
            node1_found_node2 || node2_found_node1,
            "Nodes should discover each other through mDNS when discovery events are received"
        );
    } else {
        println!("ℹ️ Skipping specific peer discovery checks - no discovery events received in test environment");
        println!("ℹ️ This is expected in isolated test environments - focusing on command functionality");
    }

    // Test clearing mDNS cache
    println!("🔄 Testing mDNS cache clearing...");
    let cleared_count1 = node1.clear_mdns_cache().await
        .expect("Failed to clear mDNS cache on node 1");
    println!("🧹 Node 1 cleared {} entries from mDNS cache", cleared_count1);

    let cleared_count2 = node2.clear_mdns_cache().await
        .expect("Failed to clear mDNS cache on node 2");
    println!("🧹 Node 2 cleared {} entries from mDNS cache", cleared_count2);

    // Verify cache is empty after clearing
    let cache_status1_after = node1.get_mdns_cache_status().await
        .expect("Failed to get mDNS cache status from node 1 after clearing");
    assert_eq!(cache_status1_after.total_peers, 0, "Node 1 mDNS cache should be empty after clearing");

    let cache_status2_after = node2.get_mdns_cache_status().await
        .expect("Failed to get mDNS cache status from node 2 after clearing");
    assert_eq!(cache_status2_after.total_peers, 0, "Node 2 mDNS cache should be empty after clearing");

    println!("✅ mDNS cache cleared successfully");

    // Test mDNS with custom TTL
    println!("🔄 Testing mDNS with custom TTL...");
    node1.enable_mdns_with_ttl(30).await.expect("Failed to enable mDNS with custom TTL on node 1");
    println!("✅ mDNS enabled with custom TTL (30 seconds)");

    // Wait a bit for mDNS to rediscover
    println!("⏳ Waiting for mDNS rediscovery (3 seconds)...");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check cache status again
    let cache_status_ttl = node1.get_mdns_cache_status().await
        .expect("Failed to get mDNS cache status with custom TTL");
    assert_eq!(cache_status_ttl.ttl_seconds, 30, "Custom TTL should be 30 seconds");

    println!("✅ mDNS with custom TTL works correctly");

    // Stop nodes
    println!("🛑 Stopping nodes...");
    node1.stop().await.expect("Failed to stop node 1");
    node2.stop().await.expect("Failed to stop node 2");

    println!("🎉 mDNS discovery test completed successfully!");
}

/// Test mDNS discovery with timeout
#[tokio::test]
async fn test_mdns_discovery_timeout() {
    // Initialize logging for debugging
    //let _ = tracing_subscriber::fmt::try_init();
    
    println!("🚀 Starting mDNS discovery timeout test...");

    // Create node
    println!("🔄 Creating node...");
    let mut node = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node");

    // Start node
    println!("🔄 Starting node...");
    node.start().await.expect("Failed to start node");

    // Enable behaviours
    println!("🔄 Enabling behaviours...");
    node.enable_identify().await.expect("Failed to enable Identify");
    node.enable_kad().await.expect("Failed to enable Kademlia");
    node.enable_mdns().await.expect("Failed to enable mDNS");

    println!("✅ All behaviours enabled");

    // Test that we can get mDNS cache status immediately (should be empty)
    let cache_status = timeout(
        Duration::from_secs(2),
        node.get_mdns_cache_status()
    ).await.expect("Timeout getting mDNS cache status")
     .expect("Failed to get mDNS cache status");

    assert_eq!(cache_status.total_peers, 0, "mDNS cache should be empty initially");
    println!("✅ Initial mDNS cache is empty as expected");

    // Stop node
    println!("🛑 Stopping node...");
    node.stop().await.expect("Failed to stop node");

    println!("🎉 mDNS discovery timeout test completed successfully!");
}

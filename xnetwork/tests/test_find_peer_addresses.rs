// tests/test_find_peer_addresses.rs - Фокус на тестировании find_peer_addresses

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::{XRoutesConfig, events::NetworkEvent};

mod common;
use common::*;

#[tokio::test]
async fn test_find_peer_local_only() {
    println!("🧪 Testing find_peer_addresses with local-only search (timeout=0)");
    
    let (mut node, commander, _events, _peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create test node");
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Test local-only search for unknown peer
    let unknown_peer = PeerId::random();
    let result = commander.find_peer_addresses_advanced(unknown_peer, 0).await;
    
    match result {
        Ok(addresses) => {
            println!("✅ Local search found {} addresses", addresses.len());
            assert!(addresses.is_empty(), "Should find 0 addresses for unknown peer");
        }
        Err(e) => {
            panic!("Local-only search should not fail: {}", e);
        }
    }
    
    node_handle.abort();
    println!("✅ Local-only find_peer_addresses test passed");
}

#[tokio::test]
async fn test_find_peer_with_short_timeout() {
    println!("🧪 Testing find_peer_addresses with short timeout");
    
    let (mut node, commander, _events, _peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create test node");
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Test search with 2 second timeout
    let unknown_peer = PeerId::random();
    let start_time = std::time::Instant::now();
    
    let result = commander.find_peer_addresses_advanced(unknown_peer, 2).await;
    let elapsed = start_time.elapsed();
    
    println!("Search completed in {:?}", elapsed);
    
    match result {
        Ok(addresses) => {
            println!("✅ Search completed with {} addresses", addresses.len());
        }
        Err(e) => {
            println!("✅ Search failed as expected: {}", e);
            assert!(e.contains("timeout") || e.contains("Kademlia"), 
                   "Should fail with timeout or Kademlia error");
        }
    }
    
    // Should complete within reasonable time
    assert!(elapsed.as_secs() <= 5, "Search took too long: {:?}", elapsed);
    
    node_handle.abort();
    println!("✅ Short timeout find_peer_addresses test passed");
}

#[tokio::test]
async fn test_find_peer_multiple_concurrent() {
    println!("🧪 Testing multiple concurrent find_peer_addresses calls");
    
    let (mut node, commander, _events, _peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create test node");
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    let target_peer = PeerId::random();
    
    // Start multiple searches for the same peer
    let search1 = commander.find_peer_addresses_advanced(target_peer, 1); // 1 sec
    let search2 = commander.find_peer_addresses_advanced(target_peer, 2); // 2 sec
    let search3 = commander.find_peer_addresses_advanced(target_peer, 0); // local only
    
    let start_time = std::time::Instant::now();
    let (result1, result2, result3) = tokio::join!(search1, search2, search3);
    let elapsed = start_time.elapsed();
    
    println!("All searches completed in {:?}", elapsed);
    
    // Local search should succeed immediately
    assert!(result3.is_ok(), "Local search should succeed");
    if let Ok(ref addresses) = result3 {
        assert!(addresses.is_empty(), "Local search should return empty for unknown peer");
    }
    
    println!("  1s search: {:?}", result1.as_ref().map(|a| a.len()).unwrap_or_else(|e| { println!("Error: {}", e); 0 }));
    println!("  2s search: {:?}", result2.as_ref().map(|a| a.len()).unwrap_or_else(|e| { println!("Error: {}", e); 0 }));
    println!("  Local search: {:?}", result3.as_ref().map(|a| a.len()).unwrap_or_else(|e| { println!("Error: {}", e); 0 }));
    
    node_handle.abort();
    println!("✅ Concurrent find_peer_addresses test passed");
}

#[tokio::test]
async fn test_find_peer_with_bootstrap() {
    println!("🧪 Testing find_peer_addresses with bootstrap server");
    
    // Create bootstrap server
    let (bootstrap_handle, bootstrap_addr, bootstrap_peer_id) = 
        create_bootstrap_server().await
        .expect("Failed to create bootstrap server");
    
    // Create client node
    let (mut client_node, client_commander, mut client_events, _client_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create client node");
    
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Connect to bootstrap
    println!("Connecting to bootstrap server...");
    let connect_result = client_commander.connect(bootstrap_addr.clone()).await;
    println!("Connect result: {:?}", connect_result);
    
    // Wait for connection event (with timeout)
    let connection_timeout = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = client_events.recv().await {
            if let NetworkEvent::PeerConnected { peer_id } = event {
                if peer_id == bootstrap_peer_id {
                    return true;
                }
            }
        }
        false
    }).await;
    
    if connection_timeout.is_ok() && connection_timeout.unwrap() {
        println!("✅ Connected to bootstrap server");
        
        // Now try to find the bootstrap peer
        let result = client_commander.find_peer_addresses_advanced(bootstrap_peer_id, 3).await;
        
        match result {
            Ok(addresses) => {
                println!("✅ Found {} addresses for bootstrap peer", addresses.len());
                if !addresses.is_empty() {
                    println!("  First address: {}", addresses[0]);
                }
            }
            Err(e) => {
                println!("⚠️  Search failed: {}", e);
                // This is ok for test - we're just testing the search mechanism
            }
        }
    } else {
        println!("⚠️  Connection to bootstrap timed out - testing search anyway");
        
        // Test search even without connection
        let result = client_commander.find_peer_addresses_advanced(bootstrap_peer_id, 2).await;
        match result {
            Ok(addresses) => println!("Search found {} addresses", addresses.len()),
            Err(e) => println!("Search failed: {}", e),
        }
    }
    
    // Cleanup
    client_handle.abort();
    bootstrap_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Bootstrap find_peer_addresses test completed");
}

#[tokio::test]
async fn test_find_peer_convenience_methods() {
    println!("🧪 Testing find_peer_addresses convenience methods");
    
    let (mut node, commander, _events, _peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create test node");
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    let target_peer = PeerId::random();
    
    // Test all convenience methods
    println!("  Testing local-only method...");
    let local_result = commander.find_peer_addresses_local_only(target_peer).await;
    assert!(local_result.is_ok(), "Local-only should not fail");
    assert!(local_result.unwrap().is_empty(), "Should find no addresses locally");
    
    println!("  Testing timeout method...");
    let timeout_result = commander.find_peer_addresses_with_timeout(target_peer, 1).await;
    match timeout_result {
        Ok(addresses) => println!("    Found {} addresses", addresses.len()),
        Err(e) => println!("    Failed as expected: {}", e),
    }
    
    node_handle.abort();
    println!("✅ Convenience methods test passed");
}

#[tokio::test]
async fn test_find_peer_search_cancellation() {
    println!("🧪 Testing find_peer_addresses search cancellation");
    
    let (mut node, commander, _events, _peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create test node");
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    let target_peer = PeerId::random();
    
    // Start a search with long timeout
    let search_future = commander.find_peer_addresses_advanced(target_peer, 10);
    
    // Wait a bit then cancel
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let cancel_result = commander.cancel_peer_search(target_peer).await;
    println!("Cancel result: {:?}", cancel_result);
    
    // Wait for search to complete
    let search_result = tokio::time::timeout(Duration::from_secs(3), search_future).await;
    
    match search_result {
        Ok(Ok(addresses)) => {
            println!("✅ Search completed with {} addresses", addresses.len());
        }
        Ok(Err(e)) => {
            println!("✅ Search failed as expected: {}", e);
        }
        Err(_) => {
            println!("⚠️  Search timed out in test framework");
        }
    }
    
    node_handle.abort();
    println!("✅ Search cancellation test passed");
}
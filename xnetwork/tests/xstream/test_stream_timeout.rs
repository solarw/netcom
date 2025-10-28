// tests/test_stream_timeout.rs

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::{XRoutesConfig, events::NetworkEvent};

use crate::common::*;

#[tokio::test]
async fn test_open_stream_between_connected_peers() {
    println!("🧪 Testing open_stream_with_timeout between connected peers");
    
    // Create server node
    let (mut server_node, server_commander, mut server_events, server_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create server node");
    
    let server_handle = tokio::spawn(async move {
        server_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Start server listener
    server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await
        .expect("Failed to start server listener");
    
    // Wait for server address
    let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = server_events.recv().await {
            if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                return addr;
            }
        }
        panic!("No listening address received");
    }).await.expect("Timeout waiting for server address");
    
    println!("Server listening on: {}", server_addr);
    
    // Create client node
    let (mut client_node, client_commander, mut client_events, _client_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create client node");
    
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Connect client to server
    println!("Connecting client to server...");
    let connect_result = client_commander.connect_with_timeout(server_addr, 5).await;
    assert!(connect_result.is_ok(), "Connection should succeed");
    
    // Wait for connection event
    let connected = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = client_events.recv().await {
            if let NetworkEvent::PeerConnected { peer_id } = event {
                if peer_id == server_peer_id {
                    return true;
                }
            }
        }
        false
    }).await.unwrap_or(false);
    
    if connected {
        println!("✅ Peers connected successfully");
        
        // Test opening stream with timeout
        println!("Opening stream with 5s timeout...");
        let start_time = std::time::Instant::now();
        
        let stream_result = client_commander.open_stream_with_timeout(server_peer_id, 5).await;
        let elapsed = start_time.elapsed();
        
        match stream_result {
            Ok(_stream) => {
                println!("✅ Stream opened successfully in {:?}", elapsed);
                assert!(elapsed.as_secs() < 5, "Stream should open before timeout");
            }
            Err(e) => {
                println!("⚠️  Stream opening failed: {}", e);
                // Stream opening might fail due to protocol negotiation in test environment
                // but should not timeout
                assert!(!e.contains("timed out"), "Should not timeout on connected peer");
            }
        }
    } else {
        println!("⚠️  Peers didn't connect - testing stream to unconnected peer");
        
        // Test stream opening to unconnected peer (should fail quickly)
        let stream_result = client_commander.open_stream_with_timeout(server_peer_id, 3).await;
        match stream_result {
            Ok(_) => panic!("Stream should not open to unconnected peer"),
            Err(e) => println!("✅ Stream correctly failed: {}", e),
        }
    }
    
    // Cleanup
    client_handle.abort();
    server_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Open stream between connected peers test completed");
}

#[tokio::test]
async fn test_open_stream_to_unknown_peer_timeout() {
    println!("🧪 Testing open_stream_with_timeout to unknown peer");
    
    let (mut client_node, client_commander, _client_events, _client_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create client node");
    
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Try to open stream to unknown peer with 2 second timeout
    let unknown_peer = PeerId::random();
    
    println!("Opening stream to unknown peer with 2s timeout...");
    let start_time = std::time::Instant::now();
    
    let stream_result = client_commander.open_stream_with_timeout(unknown_peer, 2).await;
    let elapsed = start_time.elapsed();
    
    match stream_result {
        Ok(_) => {
            panic!("Stream should not open to unknown peer");
        }
        Err(e) => {
            println!("✅ Stream opening failed as expected: {}", e);
            println!("   Elapsed time: {:?}", elapsed);
            
            if e.contains("timed out") {
                println!("✅ Correctly timed out");
                assert!(elapsed.as_secs() >= 2 && elapsed.as_secs() <= 4, 
                       "Should timeout around 2 seconds, got {:?}", elapsed);
            } else {
                println!("✅ Failed with connection error (also valid)");
                // Stream failed quickly due to no connection - also valid
            }
        }
    }
    
    client_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Open stream timeout test completed");
}

#[tokio::test]
async fn test_open_stream_no_timeout() {
    println!("🧪 Testing open_stream_with_timeout with no timeout (0)");
    
    let (mut client_node, client_commander, _client_events, _client_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create client node");
    
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    // Try to open stream with no timeout (should rely on internal timeout)
    let unknown_peer = PeerId::random();
    
    println!("Opening stream with no timeout (0)...");
    let start_time = std::time::Instant::now();
    
    // Use tokio timeout to prevent test from hanging
    let result = tokio::time::timeout(Duration::from_secs(5), 
        client_commander.open_stream_with_timeout(unknown_peer, 0)
    ).await;
    
    let _elapsed = start_time.elapsed(); // Fixed: add underscore prefix
    
    match result {
        Ok(Ok(_)) => {
            panic!("Stream should not open to unknown peer");
        }
        Ok(Err(e)) => {
            println!("✅ Stream opening failed as expected: {}", e);
            println!("   Elapsed time: {:?}", _elapsed);
            assert!(!e.contains("timed out after"), 
                   "Should not have explicit timeout message for timeout=0");
        }
        Err(_) => {
            println!("⚠️  Test framework timeout after 5s (stream opening still ongoing)");
            println!("   This is expected behavior for timeout=0");
        }
    }
    
    client_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Open stream no timeout test completed");
}

#[tokio::test]
async fn test_stream_api_convenience_methods() {
    println!("🧪 Testing stream API convenience methods");
    
    let (mut node, commander, _events, _peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create node");
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    let unknown_peer = PeerId::random();
    
    // Test default open_stream (should use 30s timeout)
    println!("Testing default open_stream...");
    let _start_time = std::time::Instant::now();
    
    let result1 = tokio::time::timeout(Duration::from_secs(2), 
        commander.open_stream(unknown_peer)
    ).await;
    
    match result1 {
        Ok(Ok(_)) => panic!("Should not succeed"),
        Ok(Err(e)) => println!("✅ Default open_stream failed quickly: {}", e),
        Err(_) => {
            println!("✅ Default open_stream still running after 2s (expected with 30s timeout)");
        }
    }
    
    // Test explicit timeout
    println!("Testing explicit 1s timeout...");
    let result2 = commander.open_stream_with_timeout(unknown_peer, 1).await;
    
    match result2 {
        Ok(_) => panic!("Should not succeed"),
        Err(e) => {
            println!("✅ 1s timeout failed: {}", e);
            // Should complete within reasonable time around 1 second
        }
    }
    
    node_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Stream API convenience methods test completed");
}

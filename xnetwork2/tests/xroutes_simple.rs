//! Simple test for XRoutes behaviour to debug command handling

use std::time::Duration;
use xnetwork2::{
    node_builder,
};

/// Simple test that just creates a node and tries to get status
#[tokio::test]
async fn test_xroutes_simple_status() {
    println!("🛠️ Starting simple XRoutes test...");
    
    // Create node with default configuration
    let mut node = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node");
    
    println!("✅ Node created successfully");
    
    // Start the node to run swarm loop
    node.start()
        .await
        .expect("Failed to start node");
    
    println!("🚀 Node started successfully");
    
    // Small delay to ensure swarm loop is running
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Get initial XRoutes status using alias method
    println!("📤 Getting XRoutes status using alias method...");
    
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    
    println!("✅ Status received: {:?}", status);
    
    // With disabled config in node_builder, identify should be enabled by default
    // but mDNS and Kademlia should be disabled
    assert!(status.identify_enabled, "Identify should be enabled initially");
    assert!(!status.mdns_enabled, "mDNS should be disabled initially");
    assert!(!status.kad_enabled, "Kademlia should be disabled initially");
    
    println!("✅ Simple XRoutes test completed successfully");
    
    // Gracefully shutdown the node
    node.force_shutdown()
        .await
        .expect("Failed to shutdown node");
    
    println!("🛑 Node shutdown completed");
}

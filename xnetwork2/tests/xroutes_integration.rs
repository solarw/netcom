//! Integration tests for XRoutes behaviour with real node

use std::time::Duration;
use xnetwork2::{
    node_builder,
};

/// Test creating a node and checking initial XRoutes status
#[tokio::test]
async fn test_xroutes_initial_status() {
    // Create node with default configuration
    let mut node = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node");
    
    // Start the node to run swarm loop
    node.start()
        .await
        .expect("Failed to start node");
    
    // Small delay to ensure swarm loop is running
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Get initial XRoutes status using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    
    // With disabled config in node_builder, all behaviours should be disabled
    assert!(!status.identify_enabled, "Identify should be disabled initially");
    assert!(!status.mdns_enabled, "mDNS should be disabled initially");
    assert!(!status.kad_enabled, "Kademlia should be disabled initially");
    
    println!("✅ Initial XRoutes status: {:?}", status);
    
    // Gracefully shutdown the node
    node.force_shutdown()
        .await
        .expect("Failed to shutdown node");
}

/// Test enabling and disabling mDNS behaviour
#[tokio::test]
async fn test_xroutes_enable_disable_mdns() {
    // Create node with default configuration
    let mut node = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node");
    
    // Start the node to run swarm loop
    node.start()
        .await
        .expect("Failed to start node");
    
    // Small delay to ensure swarm loop is running
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Enable mDNS using alias method
    node.commander.enable_mdns().await
        .expect("Failed to enable mDNS");
    
    // Small delay to allow behaviour to be enabled
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check status after enabling using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    assert!(status.mdns_enabled, "mDNS should be enabled after EnableMdns command");
    
    println!("✅ mDNS enabled successfully: {:?}", status);
    
    // Disable mDNS using alias method
    node.commander.disable_mdns().await
        .expect("Failed to disable mDNS");
    
    // Small delay to allow behaviour to be disabled
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check status after disabling using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    assert!(!status.mdns_enabled, "mDNS should be disabled after DisableMdns command");
    
    println!("✅ mDNS disabled successfully: {:?}", status);
    
    // Gracefully shutdown the node
    node.force_shutdown()
        .await
        .expect("Failed to shutdown node");
}

/// Test enabling and disabling Kademlia behaviour
#[tokio::test]
async fn test_xroutes_enable_disable_kad() {
    // Create node with default configuration
    let mut node = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node");
    
    // Start the node to run swarm loop
    node.start()
        .await
        .expect("Failed to start node");
    
    // Small delay to ensure swarm loop is running
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Enable Kademlia using alias method
    node.commander.enable_kad().await
        .expect("Failed to enable Kademlia");
    
    // Small delay to allow behaviour to be enabled
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check status after enabling using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    assert!(status.kad_enabled, "Kademlia should be enabled after EnableKad command");
    
    println!("✅ Kademlia enabled successfully: {:?}", status);
    
    // Disable Kademlia using alias method
    node.commander.disable_kad().await
        .expect("Failed to disable Kademlia");
    
    // Small delay to allow behaviour to be disabled
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check status after disabling using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    assert!(!status.kad_enabled, "Kademlia should be disabled after DisableKad command");
    
    println!("✅ Kademlia disabled successfully: {:?}", status);
    
    // Gracefully shutdown the node
    node.force_shutdown()
        .await
        .expect("Failed to shutdown node");
}

/// Test enabling both mDNS and Kademlia behaviours
#[tokio::test]
async fn test_xroutes_enable_both_behaviours() {
    // Create node with default configuration
    let mut node = node_builder::builder()
        .build()
        .await
        .expect("Failed to create node");
    
    // Start the node to run swarm loop
    node.start()
        .await
        .expect("Failed to start node");
    
    // Small delay to ensure swarm loop is running
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Enable mDNS using alias method
    node.commander.enable_mdns().await
        .expect("Failed to enable mDNS");
    
    // Enable Kademlia using alias method
    node.commander.enable_kad().await
        .expect("Failed to enable Kademlia");
    
    // Small delay to allow behaviours to be enabled
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check status after enabling both using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    
    assert!(status.mdns_enabled, "mDNS should be enabled");
    assert!(status.kad_enabled, "Kademlia should be enabled");
    assert!(!status.identify_enabled, "Identify should remain disabled");
    
    println!("✅ Both behaviours enabled successfully: {:?}", status);
    
    // Disable both behaviours using alias methods
    node.commander.disable_mdns().await
        .expect("Failed to disable mDNS");
    
    node.commander.disable_kad().await
        .expect("Failed to disable Kademlia");
    
    // Small delay to allow behaviours to be disabled
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check status after disabling both using alias method
    let status = node.commander.get_xroutes_status().await
        .expect("Failed to get XRoutes status");
    
    assert!(!status.mdns_enabled, "mDNS should be disabled");
    assert!(!status.kad_enabled, "Kademlia should be disabled");
    assert!(!status.identify_enabled, "Identify should remain disabled");
    
    println!("✅ Both behaviours disabled successfully: {:?}", status);
    
    // Gracefully shutdown the node
    node.force_shutdown()
        .await
        .expect("Failed to shutdown node");
}

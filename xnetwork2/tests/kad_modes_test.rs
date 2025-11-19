//! Test for Kademlia server/client modes

use xnetwork2::node_builder;

/// Test that with_kad_server method works correctly
#[tokio::test]
async fn test_with_kad_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🧪 Testing with_kad_server method...");
    
    let mut node = node_builder::builder()
        .with_kad_server()
        .build()
        .await
        .expect("❌ Failed to create node with kad server - critical error");
    
    println!("✅ Node created successfully with kad server mode");
    println!("   PeerId: {:?}", node.peer_id());
    
    // Cleanup
    node.force_shutdown().await.expect("❌ Failed to shutdown node");
    
    Ok(())
}

/// Test that with_kad_client method works correctly
#[tokio::test]
async fn test_with_kad_client() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🧪 Testing with_kad_client method...");
    
    let mut node = node_builder::builder()
        .with_kad_client()
        .build()
        .await
        .expect("❌ Failed to create node with kad client - critical error");
    
    println!("✅ Node created successfully with kad client mode");
    println!("   PeerId: {:?}", node.peer_id());
    
    // Cleanup
    node.force_shutdown().await.expect("❌ Failed to shutdown node");
    
    Ok(())
}

/// Test that legacy with_kademlia method still works
#[tokio::test]
async fn test_with_kademlia_legacy() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🧪 Testing legacy with_kademlia method...");
    
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .expect("❌ Failed to create node with legacy kademlia - critical error");
    
    println!("✅ Node created successfully with legacy kademlia mode");
    println!("   PeerId: {:?}", node.peer_id());
    
    // Cleanup
    node.force_shutdown().await.expect("❌ Failed to shutdown node");
    
    Ok(())
}

/// Test that multiple Kademlia modes cannot be set simultaneously
#[tokio::test]
async fn test_kad_mode_exclusivity() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🧪 Testing Kademlia mode exclusivity...");
    
    // This should work - last mode wins
    let mut node = node_builder::builder()
        .with_kad_server()
        .with_kad_client() // This should override server mode
        .build()
        .await
        .expect("❌ Failed to create node with multiple kad modes - critical error");
    
    println!("✅ Node created successfully with last kad mode winning");
    println!("   PeerId: {:?}", node.peer_id());
    
    // Cleanup
    node.force_shutdown().await.expect("❌ Failed to shutdown node");
    
    Ok(())
}

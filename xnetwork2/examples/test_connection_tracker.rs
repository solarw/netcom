//! Test example for ConnectionTracker functionality

use libp2p::{identity, Multiaddr, PeerId};
use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::{
    behaviours::xroutes::{
        connection_tracker::{ConnectionInfo, PeerConnections},
        types::{KadMode, XRoutesConfig},
    },
    node::Node,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Starting ConnectionTracker test example...");

    // Create node with ConnectionTracker enabled
    let config = XRoutesConfig::default()
        .with_connection_tracking(true)
        .with_kad(true)
        .with_identify(true)
        .with_mdns(true);

    let node = Node::new(config).await?;
    let commander = node.commander().clone();

    println!("✅ Node created with ConnectionTracker enabled");

    // Get initial connection statistics
    let stats = commander.get_connection_stats().await?;
    println!("📊 Initial connection statistics:");
    println!("  - Total connections: {}", stats.total_connections);
    println!("  - Connected peers: {}", stats.connected_peers);
    println!("  - Listen addresses: {}", stats.listen_addresses);
    println!("  - External addresses: {}", stats.external_addresses);

    // Get listen addresses
    let listen_addresses = commander.get_listen_addresses().await?;
    println!("🎧 Listen addresses:");
    for addr in &listen_addresses {
        println!("  - {}", addr);
    }

    // Get external addresses
    let external_addresses = commander.get_external_addresses().await?;
    println!("🌐 External addresses:");
    for addr in &external_addresses {
        println!("  - {}", addr);
    }

    // Get all connections (should be empty initially)
    let connections = commander.get_connections().await?;
    println!("🔗 Current connections: {}", connections.len());
    for conn in &connections {
        println!("  - Connection: {:?}", conn);
    }

    // Get connected peers (should be empty initially)
    let connected_peers = commander.get_connected_peers().await?;
    println!("👥 Connected peers: {}", connected_peers.len());
    for peer_id in &connected_peers {
        println!("  - Peer: {}", peer_id);
    }

    // Set Kademlia mode to server to test integration
    println!("🔄 Setting Kademlia mode to server...");
    commander.set_kad_mode(KadMode::Server).await?;
    println!("✅ Kademlia mode set to server");

    // Enable mDNS for local discovery
    println!("🔄 Enabling mDNS...");
    commander.enable_mdns().await?;
    println!("✅ mDNS enabled");

    // Wait a bit for network to stabilize
    println!("⏳ Waiting 5 seconds for network to stabilize...");
    sleep(Duration::from_secs(5)).await;

    // Get updated connection statistics
    let updated_stats = commander.get_connection_stats().await?;
    println!("📊 Updated connection statistics:");
    println!("  - Total connections: {}", updated_stats.total_connections);
    println!("  - Connected peers: {}", updated_stats.connected_peers);
    println!("  - Listen addresses: {}", updated_stats.listen_addresses);
    println!("  - External addresses: {}", updated_stats.external_addresses);

    // Test getting specific connection information
    if !connections.is_empty() {
        let first_connection = &connections[0];
        let connection_info = commander.get_connection(first_connection.connection_id).await?;
        println!("🔍 First connection details:");
        println!("  - Connection ID: {:?}", connection_info.connection_id);
        println!("  - Peer ID: {}", connection_info.peer_id);
        println!("  - Direction: {:?}", connection_info.direction);
        println!("  - Established: {:?}", connection_info.established_at);
        println!("  - Address: {:?}", connection_info.address);
    }

    // Test getting peer connections
    if !connected_peers.is_empty() {
        let first_peer = connected_peers[0];
        let peer_connections = commander.get_peer_connections(first_peer).await?;
        println!("👥 Peer connections for {}:", first_peer);
        println!("  - Total connections: {}", peer_connections.connections.len());
        for conn in &peer_connections.connections {
            println!("    - Connection: {:?}", conn);
        }
    }

    // Test mDNS peers
    let mdns_peers = commander.get_mdns_peers().await?;
    println!("🔍 mDNS discovered peers: {}", mdns_peers.len());
    for (peer_id, addresses) in &mdns_peers {
        println!("  - Peer: {} with {} addresses", peer_id, addresses.len());
        for addr in addresses {
            println!("    - Address: {}", addr);
        }
    }

    // Test Kademlia status
    let xroutes_status = commander.get_xroutes_status().await?;
    println!("📊 XRoutes status:");
    println!("  - Identify enabled: {}", xroutes_status.identify_enabled);
    println!("  - mDNS enabled: {}", xroutes_status.mdns_enabled);
    println!("  - Kademlia enabled: {}", xroutes_status.kad_enabled);
    println!("  - Kademlia mode: {:?}", xroutes_status.kad_mode);
    println!("  - Connection tracking: {}", xroutes_status.connection_tracking_enabled);

    // Test network state
    let network_state = commander.get_network_state().await?;
    println!("🌐 Network state:");
    println!("  - Peer ID: {}", network_state.peer_id);
    println!("  - Listening addresses: {}", network_state.listening_addresses.len());
    println!("  - Connected peers: {}", network_state.connected_peers.len());

    // Wait a bit more to observe network activity
    println!("⏳ Waiting 10 seconds to observe network activity...");
    sleep(Duration::from_secs(10)).await;

    // Final connection statistics
    let final_stats = commander.get_connection_stats().await?;
    println!("📊 Final connection statistics:");
    println!("  - Total connections: {}", final_stats.total_connections);
    println!("  - Connected peers: {}", final_stats.connected_peers);
    println!("  - Listen addresses: {}", final_stats.listen_addresses);
    println!("  - External addresses: {}", final_stats.external_addresses);

    println!("✅ ConnectionTracker test completed successfully!");

    // Shutdown the node
    println!("🛑 Shutting down node...");
    commander.shutdown().await?;
    println!("✅ Node shutdown complete");

    Ok(())
}

use clap::Parser;
use network::{commander::Commander, events::NetworkEvent, node::NetworkNode, utils::make_new_key};
use std::str::FromStr;

mod network;
use libp2p::Multiaddr;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Optional peer address to connect to (e.g. "/ip4/127.0.0.1/udp/12345/quic-v1")
    #[arg(short, long)]
    connect: Option<String>,

    /// Disable mDNS discovery
    #[arg(long, default_value_t = false)]
    disable_mdns: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();
    
    let local_key = make_new_key();
    let (mut node, cmd_tx, mut event_rx, _peer_id) = NetworkNode::new(local_key).await?;

    println!("Local peer ID: {}", node.local_peer_id());
    
    // Set mDNS state based on command line argument
    if args.disable_mdns {
        println!("mDNS discovery disabled");
        cmd_tx.send(network::commands::NetworkCommand::DisableMdns).await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send DisableMdns command: {}", e).into()
            })?;
    } else {
        println!("mDNS discovery enabled");
        cmd_tx.send(network::commands::NetworkCommand::EnableMdns).await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send EnableMdns command: {}", e).into()
            })?;
    }
    
    // Spawn the network node task
    let node_task = tokio::spawn(async move {
        node.run().await;
    });

    // Create commander
    let cmd = Commander::new(cmd_tx);
    
    // Listen on a random port (0 means OS will assign an available port)
    let port = 0;
    cmd.listen_port(port).await?;
    
    // If connect argument is provided, try to connect to that peer
    if let Some(addr_str) = args.connect {
        match Multiaddr::from_str(&addr_str) {
            Ok(addr) => {
                println!("Attempting to connect to peer at {}", addr);
                if let Err(e) = cmd.connect(addr.clone()).await {
                    eprintln!("Failed to connect to {}: {}", addr, e);
                }
            }
            Err(e) => {
                eprintln!("Invalid multiaddress format: {}", e);
            }
        }
    }
    
    // Spawn a task to handle and display network events
    let event_task = tokio::spawn(async move {
        let mut discovered_peers = std::collections::HashSet::new();
        
        while let Some(event) = event_rx.recv().await {
            match event {
                NetworkEvent::PeerConnected { peer_id } => {
                    println!("✅ Connected to peer: {}", peer_id);
                    discovered_peers.insert(peer_id);
                }
                NetworkEvent::PeerDisconnected { peer_id } => {
                    println!("❌ Disconnected from peer: {}", peer_id);
                    discovered_peers.remove(&peer_id);
                }
                NetworkEvent::ConnectionError { peer_id, error } => {
                    if let Some(pid) = peer_id {
                        println!("⚠️ Connection error with peer {}: {}", pid, error);
                    } else {
                        println!("⚠️ Connection error: {}", error);
                    }
                }
                NetworkEvent::ListeningOnAddress { addr } => {
                    println!("🔊 Listening on address: {}", addr);
                }
                NetworkEvent::MdnsIsOn {} => {
                    println!("📡 MDNS discovery is enabled");
                }
                NetworkEvent::MdnsIsOff {} => {
                    println!("📡 MDNS discovery is disabled");
                }
                NetworkEvent::ConnectionOpened { peer_id, addr, .. } => {
                    println!("🔌 Connection opened to {peer_id} at {addr}");
                }
                NetworkEvent::ConnectionClosed { peer_id, addr, .. } => {
                    println!("🔌 Connection closed to {peer_id} at {addr}");
                }
                // You can add more event handlers here as needed
                _ => {}
            }
            
            // Print the current list of discovered peers
            if !discovered_peers.is_empty() {
                println!("📋 Current discovered peers: {:?}", discovered_peers);
            }
        }
    });
    
    // Wait for the node task to complete
    let _ = tokio::join!(node_task, event_task);
    Ok(())
}
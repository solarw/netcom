#![allow(warnings)]
use std::str::FromStr;
use clap::Parser;
use network::xauth::definitions::AuthResult;
use network::xauth::por::por::{PorUtils, ProofOfRepresentation};
use network::{
    commander::Commander, events::NetworkEvent, node::NetworkNode, utils::make_new_key,
    xauth::events::PorAuthEvent,
};
use std::{collections::HashMap, time::Duration};

mod network;
use libp2p::{Multiaddr, PeerId};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Optional peer address to connect to (e.g. "/ip4/127.0.0.1/udp/12345/quic-v1")
    #[arg(short, long)]
    connect: Option<String>,

    /// Find and connect to a specific peer by PeerId using the Kademlia DHT
    #[arg(long)]
    find_peer: Option<String>,

    /// Disable mDNS discovery
    #[arg(long, default_value_t = true)]
    disable_mdns: bool,

    /// Require authentication for connections
    #[arg(long, default_value_t = false)]
    require_auth: bool,

    /// Always accept authentication requests (for testing)
    #[arg(long, default_value_t = false)]
    accept_all_auth: bool,
    
    /// Specify the port to listen on (0 = random port)
    #[arg(short, long, default_value_t = 0)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    // Create the network node's keypair
    let local_key = make_new_key();

    // Create owner keypair for PoR authentication - moved from behaviour.rs
    let owner_keypair = PorUtils::generate_owner_keypair();

    // Create a PoR valid for 24 hours
    let por = ProofOfRepresentation::create(
        &owner_keypair,
        local_key.public().to_peer_id(),
        Duration::from_secs(86400), // 24 hours
    )
    .expect("Failed to create Proof of Representation");

    // Create NetworkNode
    let (mut node, cmd_tx, mut event_rx, _peer_id) = NetworkNode::new(local_key, por).await?;
    
    let local_peer_id = node.local_peer_id(); // Save local peer_id

    println!("Local peer ID: {}", local_peer_id);

    if args.disable_mdns {
        println!("mDNS discovery disabled");
        cmd_tx
            .send(network::commands::NetworkCommand::DisableMdns)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send DisableMdns command: {}", e).into()
            })?;
    } else {
        println!("mDNS discovery enabled");
        cmd_tx
            .send(network::commands::NetworkCommand::EnableMdns)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send EnableMdns command: {}", e).into()
            })?;
    }

    // Spawn the network node task
    let node_task = tokio::spawn(async move {
        node.run().await;
    });

    // Create commander
    let cmd = Commander::new(cmd_tx.clone());

    // Listen on specified port (0 means OS will assign an available port)
    cmd.listen_port(args.port).await?;

    // If connect argument is provided, try to connect to that peer
    let mut connect = false;
    if let Some(addr_str) = args.connect {
        match Multiaddr::from_str(&addr_str) {
            Ok(addr) => {
                connect = true;
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
    
    // If find_peer argument is provided, try to find and connect to that peer using Kademlia
    let mut find_mode = false;
    if let Some(peer_id_str) = args.find_peer {
        match PeerId::from_str(&peer_id_str) {
            Ok(peer_id) => {
                find_mode = true;
                println!("Will search for peer: {} using Kademlia DHT", peer_id);
                
                // Enable Kademlia explicitly
                cmd_tx
                    .send(network::commands::NetworkCommand::EnableKad)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        format!("Failed to send EnableKad command: {}", e).into()
                    })?;
                
                // Give time for initial connections to establish
                tokio::time::sleep(Duration::from_secs(2)).await;
                
                println!("Initiating Kademlia search for peer: {}", peer_id);
                
                // Attempt multiple times to find and connect
                let mut connected = false;
                for attempt in 1..=3 {
                    println!("Attempt {} to find and connect to peer {}", attempt, peer_id);
                    
                    // Initiate a search to update the DHT with this peer's information
                    match cmd.find_peer_addresses(peer_id).await {
                        Ok(_) => println!("Kademlia search for {} initiated", peer_id),
                        Err(e) => eprintln!("Failed to start Kademlia search: {}", e),
                    }
                    
                    // Wait a bit for the search to propagate through the DHT
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    
                    // Try to find and connect to the peer
                    match cmd.find_and_connect_to_peer(peer_id).await {
                        Ok(_) => {
                            println!("Successfully found and connected to peer: {}", peer_id);
                            connected = true;
                            break;
                        }
                        Err(e) => {
                            eprintln!("Attempt {} failed to find and connect to peer: {}: {}", 
                                     attempt, peer_id, e);
                            // Wait a bit before retrying
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
                
                if !connected {
                    eprintln!("Failed to find and connect to peer after multiple attempts: {}", peer_id);
                }
            }
            Err(e) => {
                eprintln!("Invalid peer ID format: {}", e);
            }
        }
    }
    // Spawn a task to handle and display network events
    let event_task = tokio::spawn(async move {
        let mut discovered_peers = std::collections::HashSet::new();
        // Track authenticated peers separately
        let mut authenticated_peers = std::collections::HashSet::new();

        while let Some(event) = event_rx.recv().await {
            match event {
                NetworkEvent::PeerConnected { peer_id } => {
                    println!("✅ Connected to peer: {}", peer_id);
                    discovered_peers.insert(peer_id);
                }
                NetworkEvent::PeerDisconnected { peer_id } => {
                    println!("❌ Disconnected from peer: {}", peer_id);
                    discovered_peers.remove(&peer_id);
                    authenticated_peers.remove(&peer_id);
                }
                NetworkEvent::ConnectionError { peer_id, error } => {
                    if let Some(pid) = peer_id {
                        println!("⚠️ Connection error with peer {}: {}", pid, error);
                    } else {
                        println!("⚠️ Connection error: {}", error);
                    }
                }
                NetworkEvent::ListeningOnAddress { addr, full_addr } => {
                    println!("🔊 Listening on address: {}", addr);
                    if let Some(full) = full_addr {
                        println!("🌍 Full address (copy to connect): {}", full);
                    }
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
                NetworkEvent::IncomingStream { stream } => {
                    let peer_id = stream.peer_id;
                    println!("✅✅✅✅✅✅ Stream from {peer_id}");
                    let some = stream.clone().read_to_end().await;

                    let s = String::from_utf8(some.unwrap()).expect("Our bytes should be valid utf8");
                    println!("111111111111111111111111111111111 We read {} ", s);
                }
                
                // Handle Kademlia DHT events for better visibility during testing
                NetworkEvent::KadAddressAdded { peer_id, addr } => {
                    println!("📚 Kademlia address added: {peer_id} at {addr}");
                }
                
                NetworkEvent::KadRoutingUpdated { peer_id, addresses } => {
                    println!("📚 Kademlia routing updated for {peer_id}");
                    for addr in addresses {
                        println!("  - Address: {addr}");
                    }
                }

                // Handle authentication events
                NetworkEvent::AuthEvent { event } => {
                    match event {
                        PorAuthEvent::MutualAuthSuccess {
                            peer_id, metadata, ..
                        } => {
                            println!("🔐 Mutual authentication successful with peer: {peer_id}");
                            authenticated_peers.insert(peer_id);

                            // If there's metadata, print it
                            if !metadata.is_empty() {
                                println!("📝 Peer metadata: {:?}", metadata);
                            }

                            // Here you can trigger additional actions for authenticated peers
                            // For example, starting data exchange, joining swarms, etc.
                            println!("✨ Peer {peer_id} is now fully authenticated and trusted");
                            
                            // If in find mode and we've connected to a peer, open a test stream
                            if find_mode || connect {
                                match cmd.open_stream(peer_id).await {
                                    Ok(mut stream) => {
                                        println!("✅✅✅✅✅✅ Stream for {peer_id}");
                                        println!("sent {:?}", stream.write_all("Hello from kademlia test".into()).await);
                                        println!("close {:?}", stream.close().await);
                                    },
                                    Err(e) => {
                                        println!("⚠️ Failed to open stream to {peer_id}: {e}");
                                    }
                                }
                            }
                        }
                        PorAuthEvent::VerifyPorRequest {
                            peer_id,
                            connection_id,
                            address,
                            por,
                            metadata,
                        } => {
                            println!("🔍 Received authentication request from peer {peer_id}");

                            // Here we make the authentication decision
                            let auth_result = if args.accept_all_auth {
                                // If --accept-all-auth is enabled, always accept
                                println!("🔑 Automatically accepting auth request (--accept-all-auth enabled)");
                                AuthResult::Ok(HashMap::new())
                            } else {
                                // Validate the PoR
                                match por.validate() {
                                    Ok(()) => {
                                        let owner_peer_id = por.owner_public_key.to_peer_id();
                                        println!("✅ PoR validation successful for {peer_id} {owner_peer_id}");
                                        
                                        // Open test stream if we're in connect or find mode
                                        if connect || find_mode {
                                            match cmd.open_stream(peer_id).await {
                                                Ok(mut stream) => {
                                                    println!("✅✅✅✅✅✅ Stream for {peer_id} {owner_peer_id}");
                                                    println!("sent {:?}", stream.write_all("Hello from kademlia test".into()).await);
                                                    println!("close {:?}", stream.close().await);
                                                },
                                                Err(e) => println!("Failed to open stream: {e}"),
                                            }
                                        }

                                        AuthResult::Ok(HashMap::new())
                                    }
                                    Err(e) => {
                                        // Check specifically for public key errors
                                        if e.contains("Invalid owner public key") {
                                            println!("❌ PoR validation failed for {peer_id}: Invalid public key");
                                            AuthResult::Error(format!(
                                                "PoR validation failed: Invalid public key"
                                            ))
                                        } else if e.contains("expired") {
                                            println!("❌ PoR validation failed for {peer_id}: Expired PoR");
                                            AuthResult::Error(format!(
                                                "PoR validation failed: Expired"
                                            ))
                                        } else if e.contains("not yet valid") {
                                            println!("❌ PoR validation failed for {peer_id}: PoR not yet valid");
                                            AuthResult::Error(format!(
                                                "PoR validation failed: Not yet valid"
                                            ))
                                        } else if e.contains("Invalid signature") {
                                            println!("❌ PoR validation failed for {peer_id}: Invalid signature");
                                            AuthResult::Error(format!(
                                                "PoR validation failed: Invalid signature"
                                            ))
                                        } else {
                                            println!("❌ PoR validation failed for {peer_id}: {e}");
                                            AuthResult::Error(format!(
                                                "PoR validation failed: {}",
                                                e
                                            ))
                                        }
                                    }
                                }
                            };

                            // Send the authentication result back to the network using commander
                            match cmd
                                .submit_por_verification(connection_id, auth_result)
                                .await
                            {
                                Ok(_) => println!("📤 Sent authentication response for {peer_id}"),
                                Err(e) => {
                                    println!("❌ Failed to send authentication response: {e}")
                                }
                            }
                        }
                        PorAuthEvent::OutboundAuthSuccess { peer_id, .. } => {
                            println!("🔑 Outbound authentication successful with peer: {peer_id}");
                            // We verified them, but they haven't verified us yet
                            println!("⏳ Waiting for peer to verify our authentication...");
                        }
                        PorAuthEvent::InboundAuthSuccess { peer_id, .. } => {
                            println!("🔒 Inbound authentication successful with peer: {peer_id}");
                            // They verified us, but we haven't verified them yet
                            println!("⏳ Waiting to verify peer's authentication...");
                        }
                        PorAuthEvent::OutboundAuthFailure {
                            peer_id, reason, ..
                        } => {
                            println!("❌ Failed to authenticate peer {peer_id}: {reason}");
                            // Optionally disconnect from unauthenticated peers if auth is required
                            if args.require_auth {
                                println!("🚫 Disconnecting from unauthenticated peer as --require-auth is enabled");
                                // Use the commander to disconnect
                                match cmd.disconnect(peer_id).await {
                                    Ok(_) => println!("📤 Disconnected from {peer_id}"),
                                    Err(e) => println!("❌ Failed to disconnect: {e}"),
                                }
                            }
                        }
                        PorAuthEvent::InboundAuthFailure {
                            peer_id, reason, ..
                        } => {
                            println!("❌ Peer {peer_id} failed to authenticate us: {reason}");
                            // Optionally disconnect in this case as well
                            if args.require_auth {
                                println!("🚫 Disconnecting from peer that couldn't authenticate us as --require-auth is enabled");
                                // Use the commander to disconnect
                                match cmd.disconnect(peer_id).await {
                                    Ok(_) => println!("📤 Disconnected from {peer_id}"),
                                    Err(e) => println!("❌ Failed to disconnect: {e}"),
                                }
                            }
                        }
                        PorAuthEvent::AuthTimeout {
                            peer_id, direction, ..
                        } => {
                            println!(
                                "⏱️ Authentication timed out with peer {peer_id}, direction: {:?}",
                                direction
                            );
                            // Optionally disconnect for timeout
                            if args.require_auth {
                                println!("🚫 Disconnecting due to authentication timeout as --require-auth is enabled");
                                // Use the commander to disconnect
                                match cmd.disconnect(peer_id).await {
                                    Ok(_) => println!("📤 Disconnected from {peer_id}"),
                                    Err(e) => println!("❌ Failed to disconnect: {e}"),
                                }
                            }
                        }
                    }
                }

                // You can add more event handlers here as needed
                _ => {}
            }

            // Print the current list of discovered peers
            if !discovered_peers.is_empty() {
                println!("📋 Current discovered peers: {:?}", discovered_peers);
            }

            // Print the list of authenticated peers
            if !authenticated_peers.is_empty() {
                println!("🔐 Authenticated peers: {:?}", authenticated_peers);
            }
        }
    });

    // Setup ctrl+c handler to gracefully shut down
    ctrlc::set_handler(move || {
        info!("Ctrl+C received, shutting down...");
        // This will be limited because we're in a different thread,
        // but you could set up a channel to signal shutdown
        std::process::exit(0);
    })?;

    // Wait for the node task to complete
    let _ = tokio::join!(node_task, event_task);
    Ok(())
}
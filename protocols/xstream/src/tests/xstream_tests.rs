// src/tests/xstream_tests.rs

use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::{swarm::Swarm, identity, PeerId, multiaddr::Protocol, Multiaddr};
use libp2p_swarm_test::SwarmExt;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout};

use crate::{
    behaviour::XStreamNetworkBehaviour,
    events::XStreamEvent,
    xstream::XStream,
};

#[tokio::test]
async fn test_basic_xstream_read_write() {
    println!("Creating client and server nodes");
    
    // Create server swarm
    let mut server_swarm = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    let server_peer_id = *server_swarm.local_peer_id();
    println!("Server peer ID: {}", server_peer_id);
    
    // Create client swarm
    let mut client_swarm = Swarm::new_ephemeral_tokio(|_| XStreamNetworkBehaviour::new());
    let client_peer_id = *client_swarm.local_peer_id();
    println!("Client peer ID: {}", client_peer_id);
    
    // Create channels for streams
    let (server_stream_tx, mut server_stream_rx) = mpsc::channel::<XStream>(1);
    let (client_stream_tx, mut client_stream_rx) = mpsc::channel::<XStream>(1);
    
    // Channel for requesting stream opening
    let (stream_req_tx, mut stream_req_rx) = mpsc::channel::<(PeerId, oneshot::Sender<Result<XStream, String>>)>(1);
    
    // Make server listen
    let (memory_addr, _) = server_swarm.listen().with_memory_addr_external().await;
    println!("Server listening on: {}", memory_addr);
    
    // Connection flags
    let server_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let client_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    
    let server_connected_clone = server_connected.clone();
    
    // Spawn server task
    tokio::spawn(async move {
        println!("Server task started");
        
        loop {
            match server_swarm.next().await {
                Some(libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. }) => {
                    println!("Server: Connection established with {}", peer_id);
                    if peer_id == client_peer_id {
                        server_connected_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                },
                Some(libp2p::swarm::SwarmEvent::Behaviour(XStreamEvent::IncomingStream { stream })) => {
                    println!("Server: Received incoming stream from {} with ID {:?}", stream.peer_id, stream.id);
                    server_stream_tx.send(stream).await
                        .expect("Failed to send server stream to test");
                    println!("Server: Successfully sent stream to test");
                },
                Some(event) => {
                    println!("Server event: {:?}", event);
                },
                None => break,
            }
        }
    });
    
    let client_connected_clone = client_connected.clone();
    
    // Spawn client task
    tokio::spawn(async move {
        println!("Client task started");
        
        // Connect to server
        println!("Client connecting to server at {}", memory_addr);
        if let Err(e) = client_swarm.dial(memory_addr.clone()) {
            panic!("Client failed to dial: {:?}", e);
        }
        
        loop {
            tokio::select! {
                // Process swarm events
                event = client_swarm.next() => {
                    match event {
                        Some(libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. }) => {
                            println!("Client: Connection established with {}", peer_id);
                            if peer_id == server_peer_id {
                                client_connected_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                        },
                        Some(libp2p::swarm::SwarmEvent::Behaviour(XStreamEvent::StreamEstablished { peer_id, stream_id })) => {
                            println!("Client: Stream established with {} (ID: {:?})", peer_id, stream_id);
                        },
                        Some(event) => {
                            println!("Client event: {:?}", event);
                        },
                        None => break,
                    }
                },
                
                // Process stream opening requests
                request = stream_req_rx.recv() => {
                    if let Some((peer_id, response_channel)) = request {
                        println!("Client: Received request to open stream to {}", peer_id);
                        client_swarm.behaviour_mut().open_stream(peer_id, response_channel).await;
                        println!("Client: Stream open request sent");
                    } else {
                        panic!("Client: Stream request channel closed unexpectedly");
                    }
                }
            }
        }
    });
    
    // Wait for connections to be established
    println!("Waiting for connections to be established...");
    let mut attempts = 0;
    const MAX_ATTEMPTS: usize = 20;
    
    while (!server_connected.load(std::sync::atomic::Ordering::SeqCst) ||
           !client_connected.load(std::sync::atomic::Ordering::SeqCst)) && attempts < MAX_ATTEMPTS {
        sleep(Duration::from_millis(100)).await;
        attempts += 1;
    }
    
    if !server_connected.load(std::sync::atomic::Ordering::SeqCst) {
        panic!("Server did not connect to client after {} attempts", MAX_ATTEMPTS);
    }
    if !client_connected.load(std::sync::atomic::Ordering::SeqCst) {
        panic!("Client did not connect to server after {} attempts", MAX_ATTEMPTS);
    }
    
    println!("Both sides connected successfully!");
    
    // Wait a moment to ensure connection stability
    sleep(Duration::from_millis(300)).await;
    
    // Request to open a stream
    println!("Requesting client to open stream to server");
    let (tx, rx) = oneshot::channel();
    
    stream_req_tx.send((server_peer_id, tx)).await
        .expect("Failed to send stream open request");
    println!("Stream open request sent to client task");
    
    // Wait for the stream from the oneshot channel
    println!("Waiting for client stream from oneshot channel");
    let client_stream = match timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(stream))) => {
            println!("Got client stream from oneshot with ID {:?}!", stream.id);
            stream
        },
        Ok(Ok(Err(e))) => panic!("Error opening stream: {}", e),
        Ok(Err(_)) => panic!("Oneshot channel closed unexpectedly"),
        Err(_) => panic!("Timeout waiting for oneshot response after 5 seconds"),
    };
    
    // Forward it to the test
    client_stream_tx.send(client_stream).await
        .expect("Failed to forward client stream to test");
    
    // Wait for server stream
    println!("Waiting for server stream...");
    let server_stream = timeout(Duration::from_secs(5), server_stream_rx.recv()).await
        .expect("Timeout waiting for server stream after 5 seconds")
        .expect("Server stream channel closed unexpectedly");
    println!("Got server stream with ID {:?}!", server_stream.id);
    
    // Get client stream from channel
    println!("Getting client stream from channel...");
    let client_stream = timeout(Duration::from_secs(1), client_stream_rx.recv()).await
        .expect("Timeout waiting for client stream from channel")
        .expect("Client stream channel closed unexpectedly");
    println!("Got client stream from channel!");
    
    // Test data transfer
    println!("Testing data transfer...");
    let test_data = b"Hello, XStream!".to_vec();
    
    // Client to server
    println!("Client writing data to server...");
    client_stream.write_all(test_data.clone()).await
        .expect("Failed to write data from client");
    println!("Client wrote data successfully");
    
    // Server reads
    println!("Server reading data from client...");
    let received_data = server_stream.read_exact(test_data.len()).await
        .expect("Failed to read data on server");
    println!("Server read data successfully");
    
    // Verify data
    assert_eq!(received_data, test_data, "Data mismatch: client->server");
    println!("Client->server data verified");
    
    // Server to client
    let response_data = b"Response from server!".to_vec();
    println!("Server writing data to client...");
    server_stream.write_all(response_data.clone()).await
        .expect("Failed to write data from server");
    println!("Server wrote data successfully");
    
    // Client reads
    println!("Client reading data from server...");
    let client_received = client_stream.read_exact(response_data.len()).await
        .expect("Failed to read data on client");
    println!("Client read data successfully");
    
    // Verify data
    assert_eq!(client_received, response_data, "Data mismatch: server->client");
    println!("Server->client data verified");
    
    println!("Test completed successfully");
}
// src/tests/basic_echo_test.rs

use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::{PeerId, swarm::Swarm};
use libp2p_swarm_test::SwarmExt;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout};

use crate::{behaviour::XStreamNetworkBehaviour, events::XStreamEvent, xstream::XStream};

// Structure to hold paired streams for testing
struct XStreamTestPair {
    client_stream: XStream,
    server_stream: XStream,
    client_peer_id: PeerId,
    server_peer_id: PeerId,
}

// Helper struct to manage shutdown
struct ShutdownManager {
    client_shutdown: mpsc::Sender<()>,
    server_shutdown: mpsc::Sender<()>,
}

impl ShutdownManager {
    // Shutdown tasks in the correct order
    async fn shutdown(&self) {
        println!("Starting coordinated shutdown...");
        // First close the client, then the server
        let _ = self.client_shutdown.send(()).await;
        // Wait a bit for client to close
        sleep(Duration::from_millis(100)).await;
        // Then close the server
        let _ = self.server_shutdown.send(()).await;
        // Wait for everything to close properly
        sleep(Duration::from_millis(200)).await;
        println!("Coordinated shutdown complete");
    }
}

// Function to create a pair of streams for testing
async fn create_xstream_test_pair() -> (XStreamTestPair, ShutdownManager) {
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
    let (stream_req_tx, mut stream_req_rx) =
        mpsc::channel::<(PeerId, oneshot::Sender<Result<XStream, String>>)>(1);

    // Create shutdown channels for both server and client tasks
    let (server_shutdown_tx, mut server_shutdown_rx) = mpsc::channel::<()>(1);
    let (client_shutdown_tx, mut client_shutdown_rx) = mpsc::channel::<()>(1);

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
            tokio::select! {
                // Handle shutdown signal
                _ = server_shutdown_rx.recv() => {
                    println!("Server received shutdown signal, exiting");
                    break;
                },

                // Process swarm events
                event = server_swarm.next() => {
                    match event {
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
            }
        }

        println!("Server task exiting");
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
                // Handle shutdown signal
                _ = client_shutdown_rx.recv() => {
                    println!("Client received shutdown signal, exiting");
                    break;
                },

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
                        None => {
                            println!("Client swarm returned None, exiting");
                            break;
                        }
                    }
                },

                // Process stream opening requests
                request = stream_req_rx.recv() => {
                    match request {
                        Some((peer_id, response_channel)) => {
                            println!("Client: Received request to open stream to {}", peer_id);
                            client_swarm.behaviour_mut().open_stream(peer_id, response_channel).await;
                            println!("Client: Stream open request sent");
                        },
                        None => {
                            // Create a new channel that will never be consumed, just to keep select! happy
                            let (_, new_rx) = mpsc::channel::<(PeerId, oneshot::Sender<Result<XStream, String>>)>(1);
                            stream_req_rx = new_rx;
                            println!("Client: Stream request channel closed, continuing to run");
                        }
                    }
                }
            }
        }

        println!("Client task exiting");
    });

    // Wait for connections to be established
    println!("Waiting for connections to be established...");
    let mut attempts = 0;
    const MAX_ATTEMPTS: usize = 20;

    while (!server_connected.load(std::sync::atomic::Ordering::SeqCst)
        || !client_connected.load(std::sync::atomic::Ordering::SeqCst))
        && attempts < MAX_ATTEMPTS
    {
        sleep(Duration::from_millis(100)).await;
        attempts += 1;
    }

    if !server_connected.load(std::sync::atomic::Ordering::SeqCst) {
        panic!(
            "Server did not connect to client after {} attempts",
            MAX_ATTEMPTS
        );
    }
    if !client_connected.load(std::sync::atomic::Ordering::SeqCst) {
        panic!(
            "Client did not connect to server after {} attempts",
            MAX_ATTEMPTS
        );
    }

    println!("Both sides connected successfully!");

    // Wait a moment to ensure connection stability
    sleep(Duration::from_millis(300)).await;

    // Request to open a stream
    println!("Requesting client to open stream to server");
    let (tx, rx) = oneshot::channel();

    stream_req_tx
        .send((server_peer_id, tx))
        .await
        .expect("Failed to send stream open request");
    println!("Stream open request sent to client task");

    // Wait for the stream from the oneshot channel
    println!("Waiting for client stream from oneshot channel");
    let client_stream = match timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(stream))) => {
            println!("Got client stream from oneshot with ID {:?}!", stream.id);
            stream
        }
        Ok(Ok(Err(e))) => panic!("Error opening stream: {}", e),
        Ok(Err(_)) => panic!("Oneshot channel closed unexpectedly"),
        Err(_) => panic!("Timeout waiting for oneshot response after 5 seconds"),
    };

    // Forward it to the test
    client_stream_tx
        .send(client_stream)
        .await
        .expect("Failed to forward client stream to test");

    // Wait for server stream
    println!("Waiting for server stream...");
    let server_stream = timeout(Duration::from_secs(5), server_stream_rx.recv())
        .await
        .expect("Timeout waiting for server stream after 5 seconds")
        .expect("Server stream channel closed unexpectedly");
    println!("Got server stream with ID {:?}!", server_stream.id);

    // Get client stream from channel
    println!("Getting client stream from channel...");
    let client_stream = timeout(Duration::from_secs(1), client_stream_rx.recv())
        .await
        .expect("Timeout waiting for client stream from channel")
        .expect("Client stream channel closed unexpectedly");
    println!("Got client stream from channel!");

    // Create a ShutdownManager to handle proper shutdown coordination
    let shutdown_manager = ShutdownManager {
        client_shutdown: client_shutdown_tx,
        server_shutdown: server_shutdown_tx,
    };

    // Return the test pair and shutdown manager
    (
        XStreamTestPair {
            client_stream,
            server_stream,
            client_peer_id,
            server_peer_id,
        },
        shutdown_manager,
    )
}

// Basic echo test: client sends data to server, server sends it back, and client verifies
#[tokio::test]
async fn test_basic_echo() {
    // Create the test pair
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test data
    let test_data = b"Hello, XStream Echo Test!".to_vec();
    println!("Testing basic echo with data: {:?}", test_data);

    // Step 1: Client writes data to server
    println!("Client writing data to server...");
    test_pair
        .client_stream
        .write_all(test_data.clone())
        .await
        .expect("Failed to write data from client");

    // Force a flush to ensure all data is sent
    test_pair
        .client_stream
        .flush()
        .await
        .expect("Failed to flush client stream");

    // Step 2: Server reads the data
    println!("Server reading data from client...");

    let received_data = test_pair
        .server_stream
        .read_exact(test_data.len())
        .await
        .expect("some data read");

    // Verify server received the data correctly
    assert!(!received_data.is_empty(), "Server couldn't read any data");
    assert_eq!(received_data, test_data, "Data mismatch: client->server");
    println!("Server received data correctly: {:?}", received_data);

    // Step 3: Server echoes the data back
    println!("Server echoing data back to client...");
    test_pair
        .server_stream
        .write_all(received_data.clone())
        .await
        .expect("Failed to echo data from server");

    // Force a flush to ensure all data is sent
    test_pair
        .server_stream
        .flush()
        .await
        .expect("Failed to flush server stream");

    // Step 4: Client reads the echoed data
    println!("Client reading echoed data...");

    let echoed_data = test_pair
        .client_stream
        .read_exact(test_data.len())
        .await
        .expect("failed to read data echoed back");

    // Verify client received the echoed data correctly
    assert!(
        !echoed_data.is_empty(),
        "Client couldn't read any echoed data"
    );
    assert_eq!(
        echoed_data, test_data,
        "Data mismatch in echo: server->client"
    );
    println!("Client received echoed data correctly: {:?}", echoed_data);

    println!("Basic echo test completed successfully!");

    // Perform coordinated shutdown
    shutdown_manager.shutdown().await;
}

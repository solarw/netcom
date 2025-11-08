// read_to_end_close_quic_test.rs
// Test to reproduce the issue where read_to_end hangs when close is called without write_eof
// Using real QUIC transport instead of test protocol

use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::{identity, quic, swarm::{SwarmEvent, dial_opts::DialOpts}, Multiaddr, PeerId};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use crate::behaviour::XStreamNetworkBehaviour;
use crate::xstream::XStream;
use crate::events::XStreamEvent;


/// Helper function to create a QUIC swarm with XStream behaviour
async fn create_quic_swarm() -> (libp2p::Swarm<XStreamNetworkBehaviour>, PeerId) {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();
    
    // Create QUIC transport
    let quic_config = quic::Config::new(&keypair);
    let quic_transport = quic::tokio::Transport::new(quic_config);
    
    // Create swarm with XStream behaviour
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|_key| quic_transport)
        .expect("Failed to create QUIC transport")
        .with_behaviour(|_key| XStreamNetworkBehaviour::new())
        .expect("Failed to create XStream behaviour")
        .build();
    
    (swarm, peer_id)
}

/// Helper function to wait for a listen address from a swarm
async fn wait_for_listen_addr(swarm: &mut libp2p::Swarm<XStreamNetworkBehaviour>) -> Multiaddr {
    timeout(Duration::from_secs(2), async {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                return address;
            }
        }
    })
    .await
    .expect("Timeout waiting for listen address")
}

/// Test that verifies the fix: read_to_end works when write_eof is called before close
/// Using real QUIC transport
#[tokio::test]
async fn test_read_to_end_works_with_write_eof_quic() {
    println!("🚀 Starting read_to_end with write_eof test with QUIC transport (verifying fix)...");
    
    // Test data
    let test_data = b"Test data for read_to_end with write_eof and QUIC".to_vec();
    
    println!("Testing with data: {:?}", test_data);

    // Create two swarms (client and server) with QUIC transport
    let (mut client_swarm, client_peer_id) = create_quic_swarm().await;
    let (mut server_swarm, server_peer_id) = create_quic_swarm().await;

    println!("✅ Swarms created:");
    println!("   Client PeerId: {}", client_peer_id);
    println!("   Server PeerId: {}", server_peer_id);

    // Start server listening
    let server_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    server_swarm.listen_on(server_addr.clone()).expect("Failed to listen");
    println!("✅ Server listening on: {}", server_addr);

    // Get the actual listen address
    let listen_addr = wait_for_listen_addr(&mut server_swarm).await;
    println!("✅ Server actually listening on: {}", listen_addr);

    // Create oneshot channels for stream communication
    let (server_stream_tx, server_stream_rx) = tokio::sync::oneshot::channel();
    let (client_stream_tx, client_stream_rx) = tokio::sync::oneshot::channel();

    // Create shutdown channels
    let (server_shutdown_tx, mut server_shutdown_rx) = tokio::sync::mpsc::channel(1);
    let (client_shutdown_tx, mut client_shutdown_rx) = tokio::sync::mpsc::channel(1);

    // Spawn server task - only handles events and passes stream through oneshot
    let server_task = tokio::spawn({
        let server_peer_id = server_peer_id.clone();
        let client_peer_id = client_peer_id.clone();
        let mut server_stream_tx = Some(server_stream_tx);
        async move {
            println!("🎯 Server task started, waiting for incoming connection and stream...");
            
            loop {
                tokio::select! {
                    event = server_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::NewListenAddr { address, .. } => {
                                println!("📡 Server listening on: {}", address);
                            }
                            SwarmEvent::IncomingConnection { connection_id, .. } => {
                                println!("🔗 Server: Incoming connection: {:?}", connection_id);
                            }
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Server: Connection established with: {}", peer_id);
                                if peer_id == client_peer_id {
                                    println!("✅ Server: Connected to expected client");
                                }
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::IncomingStream { stream } => {
                                        println!("📥 Server: Received incoming XStream");
                                        // Send stream through oneshot and break
                                        if let Some(tx) = server_stream_tx.take() {let _ = tx.send(stream);}
                                        
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = server_shutdown_rx.recv() => {
                        println!("🛑 Server task received shutdown signal");
                        break;
                    }
                }
            }
            println!("🛑 Server task completed");
        }
    });

    // Spawn client task - only handles events and passes stream through oneshot
    let client_task = tokio::spawn({
        let client_peer_id = client_peer_id.clone();
        let server_peer_id = server_peer_id.clone();
        let mut client_stream_tx = Some(client_stream_tx);
        async move {
            println!("🎯 Client task started, connecting to server...");
            
            // Wait a bit for server to start listening
            sleep(Duration::from_millis(100)).await;

            // Connect to server
            client_swarm.dial(DialOpts::peer_id(server_peer_id).addresses(vec![listen_addr.clone()]).build()).unwrap();
            println!("🔗 Client: Dialing server at {}", listen_addr);
            loop {
                tokio::select! {
                    event = client_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Client: Connection established with: {}", peer_id);
                                if peer_id == server_peer_id{
                                    println!("✅ Client: Connected to expected server");
                                    
                                    // Open XStream to server using oneshot channel
                                    println!("🔄 Client: Opening XStream to server...");
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    client_swarm.behaviour_mut().open_stream(server_peer_id, tx).await;
                                    if let Some(tx) = client_stream_tx.take() {
                                        println!("🔄 Client: Opening XStream to server... SENDING OVER");
                                       let _ = tx.send(rx);
                                    }
                                    
                                }
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                                        println!("✅ Client: XStream established to: {}", peer_id);
                                        // We'll handle this in the open_stream response
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = client_shutdown_rx.recv() => {
                        println!("🛑 Client task received shutdown signal");
                        break;
                    }
                }
            }
            println!("🛑 Client task completed");
        }
    });

    let recv_result = timeout(Duration::from_secs(10), client_stream_rx).await;
    let client_stream_rx = match recv_result {
        Ok(Ok(rx)) => rx, // получили Ok(rx) — сам oneshot::Receiver (или ваш тип)
        Ok(Err(_recv_err)) => {
            panic!("client_stream_rx Sender dropped before sending the client stream (RecvError)");
        }
        Err(_) => {
            panic!("client_stream_rx Timeout waiting for oneshot sender to send the client stream");
        }
    };
    let maybe_client_stream = timeout(Duration::from_secs(10), client_stream_rx).await;
    let client_stream = match maybe_client_stream {
        Ok(Ok(rx)) => {
            // rx — это, возможно, ещё один Result (в зависимости от того, что вы передаете)
            // если oneshot передаёт Option/Result — распакуйте дополнительно
            match rx {
                Ok(stream) => stream, // если send(stream) отправлял Ok(stream)
                Err(e) => panic!("client_stream oneshot inner error when receiving client stream: {:?}", e),
            }
        }
        Ok(Err(_recv_err)) => {
            panic!("client_stream Sender dropped before sending client stream (RecvError) ");
        }
        Err(_) => {
            panic!("client_stream Timeout waiting for client stream");
        }
    };
    
    let server_stream = timeout(Duration::from_secs(5), server_stream_rx)
        .await
        .expect("Timeout waiting for server stream")
        .expect("Failed to get server stream");

    println!("✅ Both XStreams established, starting data transfer test...");

    // Now we have both streams, perform parallel operations in main test body
    let mut client_stream = client_stream;
    let server_stream = server_stream;

    // Clone test_data for both tasks
    let client_test_data = test_data.clone();
    let server_test_data = test_data.clone();

    // Spawn client operations task - write data, send EOF, and close
    let client_handle = tokio::spawn(async move {
        println!("📤 Client writing data to server...");
        sleep(Duration::from_millis(50)).await;
        client_stream
            .write_all(client_test_data)
            .await
            .expect("Failed to write data from client");

        // Force a flush to ensure all data is sent
        
        client_stream
            .flush()
            .await
            .expect("Failed to flush client stream");

        println!("✅ Data written and flushed");

        // Step 2: Client sends EOF before closing (this should work correctly)
        println!("📝 Client calling write_eof before close...");
        
        client_stream
            .write_eof()
            .await
            .expect("Failed to write EOF from client");
        
        println!("✅ EOF written");

        // Step 3: Client closes the stream
        println!("🔒 Client closing stream...");
        client_stream.close().await.expect("Failed to close client stream");
        println!("✅ Client stream closed");
        sleep(Duration::from_millis(300)).await;
    });

    // In main test body: wait a bit and then try read_to_end with timeout
    sleep(Duration::from_millis(10)).await;

    // Step 4: Server reads all data with read_to_end
    // This should work because EOF was sent
    println!("📥 Server attempting read_to_end (should work with EOF)...");
    
    // Use timeout to ensure it doesn't hang
    let read_result = timeout(Duration::from_millis(200), server_stream.read_to_end()).await;
    
    match read_result {
        Ok(Ok(data)) => {
            // This is the expected behavior - read_to_end completed successfully
            println!("✅ EXPECTED: read_to_end completed successfully with data: {:?}", data);
            assert_eq!(data, server_test_data, "Data mismatch in read_to_end");
            println!("✅ Test verified that read_to_end works with write_eof!");
        }
        Ok(Err(e)) => {
            println!("❌ UNEXPECTED: read_to_end returned error: {:?}", e);
            panic!("read_to_end should have worked with EOF, but returned error: {:?}", e);
        }
        Err(_) => {
            println!("❌ UNEXPECTED: read_to_end timed out even with EOF");
            panic!("read_to_end should have completed with EOF, but it timed out");
        }
    }

    // Wait for client operations to complete
    let _ = client_handle.await;

    // Send shutdown signal to tasks
    let _ = server_shutdown_tx.send(()).await;
    let _ = client_shutdown_tx.send(()).await;

    // Wait for tasks to complete
    let _ = tokio::join!(client_task, server_task);

    println!("✅ QUIC transport test completed successfully!");
}



/// Test that verifies the fix: read_to_end  DOES NOT works when NO write_eof is called before close
/// Using real QUIC transport
#[tokio::test]
async fn test_read_to_end_works_with_write_NO_eof_quic() {
    println!("🚀 Starting read_to_end with write_eof test with QUIC transport (verifying fix)...");
    
    // Test data
    let test_data = b"Test data for read_to_end with write_eof and QUIC".to_vec();
    
    println!("Testing with data: {:?}", test_data);

    // Create two swarms (client and server) with QUIC transport
    let (mut client_swarm, client_peer_id) = create_quic_swarm().await;
    let (mut server_swarm, server_peer_id) = create_quic_swarm().await;

    println!("✅ Swarms created:");
    println!("   Client PeerId: {}", client_peer_id);
    println!("   Server PeerId: {}", server_peer_id);

    // Start server listening
    let server_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    server_swarm.listen_on(server_addr.clone()).expect("Failed to listen");
    println!("✅ Server listening on: {}", server_addr);

    // Get the actual listen address
    let listen_addr = wait_for_listen_addr(&mut server_swarm).await;
    println!("✅ Server actually listening on: {}", listen_addr);

    // Create oneshot channels for stream communication
    let (server_stream_tx, server_stream_rx) = tokio::sync::oneshot::channel();
    let (client_stream_tx, client_stream_rx) = tokio::sync::oneshot::channel();

    // Create shutdown channels
    let (server_shutdown_tx, mut server_shutdown_rx) = tokio::sync::mpsc::channel(1);
    let (client_shutdown_tx, mut client_shutdown_rx) = tokio::sync::mpsc::channel(1);

    // Spawn server task - only handles events and passes stream through oneshot
    let server_task = tokio::spawn({
        let server_peer_id = server_peer_id.clone();
        let client_peer_id = client_peer_id.clone();
        let mut server_stream_tx = Some(server_stream_tx);
        async move {
            println!("🎯 Server task started, waiting for incoming connection and stream...");
            
            loop {
                tokio::select! {
                    event = server_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::NewListenAddr { address, .. } => {
                                println!("📡 Server listening on: {}", address);
                            }
                            SwarmEvent::IncomingConnection { connection_id, .. } => {
                                println!("🔗 Server: Incoming connection: {:?}", connection_id);
                            }
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Server: Connection established with: {}", peer_id);
                                if peer_id == client_peer_id {
                                    println!("✅ Server: Connected to expected client");
                                }
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::IncomingStream { stream } => {
                                        println!("📥 Server: Received incoming XStream");
                                        // Send stream through oneshot and break
                                        if let Some(tx) = server_stream_tx.take() {let _ = tx.send(stream);}
                                        
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = server_shutdown_rx.recv() => {
                        println!("🛑 Server task received shutdown signal");
                        break;
                    }
                }
            }
            println!("🛑 Server task completed");
        }
    });

    // Spawn client task - only handles events and passes stream through oneshot
    let client_task = tokio::spawn({
        let client_peer_id = client_peer_id.clone();
        let server_peer_id = server_peer_id.clone();
        let mut client_stream_tx = Some(client_stream_tx);
        async move {
            println!("🎯 Client task started, connecting to server...");
            
            // Wait a bit for server to start listening
            sleep(Duration::from_millis(100)).await;

            // Connect to server
            client_swarm.dial(DialOpts::peer_id(server_peer_id).addresses(vec![listen_addr.clone()]).build()).unwrap();
            println!("🔗 Client: Dialing server at {}", listen_addr);
            loop {
                tokio::select! {
                    event = client_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Client: Connection established with: {}", peer_id);
                                if peer_id == server_peer_id{
                                    println!("✅ Client: Connected to expected server");
                                    
                                    // Open XStream to server using oneshot channel
                                    println!("🔄 Client: Opening XStream to server...");
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    client_swarm.behaviour_mut().open_stream(server_peer_id, tx).await;
                                    if let Some(tx) = client_stream_tx.take() {
                                        println!("🔄 Client: Opening XStream to server... SENDING OVER");
                                       let _ = tx.send(rx);
                                    }
                                    
                                }
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                                        println!("✅ Client: XStream established to: {}", peer_id);
                                        // We'll handle this in the open_stream response
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = client_shutdown_rx.recv() => {
                        println!("🛑 Client task received shutdown signal");
                        break;
                    }
                }
            }
            println!("🛑 Client task completed");
        }
    });

    let recv_result = timeout(Duration::from_secs(10), client_stream_rx).await;
    let client_stream_rx = match recv_result {
        Ok(Ok(rx)) => rx, // получили Ok(rx) — сам oneshot::Receiver (или ваш тип)
        Ok(Err(_recv_err)) => {
            panic!("client_stream_rx Sender dropped before sending the client stream (RecvError)");
        }
        Err(_) => {
            panic!("client_stream_rx Timeout waiting for oneshot sender to send the client stream");
        }
    };
    let maybe_client_stream = timeout(Duration::from_secs(10), client_stream_rx).await;
    let client_stream = match maybe_client_stream {
        Ok(Ok(rx)) => {
            // rx — это, возможно, ещё один Result (в зависимости от того, что вы передаете)
            // если oneshot передаёт Option/Result — распакуйте дополнительно
            match rx {
                Ok(stream) => stream, // если send(stream) отправлял Ok(stream)
                Err(e) => panic!("client_stream oneshot inner error when receiving client stream: {:?}", e),
            }
        }
        Ok(Err(_recv_err)) => {
            panic!("client_stream Sender dropped before sending client stream (RecvError) ");
        }
        Err(_) => {
            panic!("client_stream Timeout waiting for client stream");
        }
    };
    
    let server_stream = timeout(Duration::from_secs(5), server_stream_rx)
        .await
        .expect("Timeout waiting for server stream")
        .expect("Failed to get server stream");

    println!("✅ Both XStreams established, starting data transfer test...");

    // Now we have both streams, perform parallel operations in main test body
    let mut client_stream = client_stream;
    let server_stream = server_stream;

    // Clone test_data for both tasks
    let client_test_data = test_data.clone();
    let server_test_data = test_data.clone();

    // Spawn client operations task - write data, send EOF, and close
    let client_handle = tokio::spawn(async move {
        println!("📤 Client writing data to server...");
        sleep(Duration::from_millis(50)).await;
        client_stream
            .write_all(client_test_data)
            .await
            .expect("Failed to write data from client");

        // Force a flush to ensure all data is sent
        
        client_stream
            .flush()
            .await
            .expect("Failed to flush client stream");

        println!("✅ Data written and flushed");

        // Step 2: Client DONT sends EOF before closing (this should work correctly)
        println!("📝 Client DONT calling write_eof before close...");
        
        // Step 3: Client closes the stream
        println!("🔒 Client closing stream...");
        client_stream.close().await.expect("Failed to close client stream");
        println!("✅ Client stream closed");
        sleep(Duration::from_millis(300)).await;
    });

    // In main test body: wait a bit and then try read_to_end with timeout
    sleep(Duration::from_millis(10)).await;

    // Step 4: Server reads all data with read_to_end
    // This should NOT work because EOF was NOT sent (we fixed close() to send EOF automatically)
    println!("📥 Server attempting read_to_end (should timeout because close() now sends EOF automatically)...");
    
    // Use timeout to ensure it doesn't hang - this should timeout because close() now sends EOF
    let read_result = timeout(Duration::from_millis(200), server_stream.read_to_end()).await;
    
    match read_result {
        Ok(Ok(data)) => {
            // This is the NEW expected behavior - close() now automatically sends EOF
            println!("✅ EXPECTED: read_to_end completed successfully with data: {:?}", data);
            assert_eq!(data, server_test_data, "Data mismatch in read_to_end");
            println!("✅ Test verified that close() now automatically sends EOF!");
        }
        Ok(Err(e)) => {
            println!("❌ UNEXPECTED: read_to_end returned error: {:?}", e);
            panic!("read_to_end should have worked with automatic EOF from close(), but returned error: {:?}", e);
        }
        Err(_) => {
            println!("❌ UNEXPECTED: read_to_end timed out even with automatic EOF from close()");
            panic!("read_to_end should have completed with automatic EOF from close(), but it timed out");
        }
    }

    // Wait for client operations to complete
    let _ = client_handle.await;

    // Send shutdown signal to tasks
    let _ = server_shutdown_tx.send(()).await;
    let _ = client_shutdown_tx.send(()).await;

    // Wait for tasks to complete
    let _ = tokio::join!(client_task, server_task);

    println!("✅ QUIC transport test completed successfully!");
}

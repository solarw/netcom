//! QUIC тесты для close() функциональности в XStream
//! Проверяет поведение close() с QUIC транспортом
//! 
//! ВАЖНО: Используем close() вместо close_read() из-за гонок в QUIC
//! - close() атомарно закрывает обе половины потока (read и write)
//! - Уведомляет транспортный уровень о полном закрытии
//! - Задержка необходима для распространения состояния в QUIC event loop
//! - В реальной сети QUIC требуется время для обработки закрытия

use libp2p::{identity, quic, Multiaddr, PeerId, swarm::{Swarm, SwarmEvent, dial_opts::DialOpts}};
use libp2p::futures::StreamExt;
use tokio::sync::{oneshot, mpsc};
use std::time::Duration;
use tokio::time::{sleep, timeout};

use crate::{behaviour::XStreamNetworkBehaviour, events::XStreamEvent, xstream::XStream};

// Structure to hold paired streams for QUIC testing
pub struct XStreamQuicTestPair {
    pub client_stream: XStream,
    pub server_stream: XStream,
    pub client_peer_id: PeerId,
    pub server_peer_id: PeerId,
}

// Helper struct to manage shutdown for QUIC
pub struct QuicShutdownManager {
    pub client_shutdown: mpsc::Sender<()>,
    pub server_shutdown: mpsc::Sender<()>,
}

impl QuicShutdownManager {
    // Shutdown tasks in the correct order
    pub async fn shutdown(&self) {
        println!("Starting coordinated QUIC shutdown...");
        // First close the client, then the server
        let _ = self.client_shutdown.send(()).await;
        // Wait a bit for client to close
        sleep(Duration::from_millis(100)).await;
        // Then close the server
        let _ = self.server_shutdown.send(()).await;
        // Wait for everything to close properly
        sleep(Duration::from_millis(200)).await;
        println!("Coordinated QUIC shutdown complete");
    }
}

// Function to create a pair of QUIC streams for testing
pub async fn create_xstream_quic_test_pair() -> (XStreamQuicTestPair, QuicShutdownManager) {
    println!("Creating QUIC client and server nodes");

    // Create server swarm with QUIC
    let (mut server_swarm, server_peer_id) = create_quic_swarm().await;
    println!("QUIC Server peer ID: {}", server_peer_id);

    // Create client swarm with QUIC
    let (mut client_swarm, client_peer_id) = create_quic_swarm().await;
    println!("QUIC Client peer ID: {}", client_peer_id);

    // Create channels for streams
    let (server_stream_tx, mut server_stream_rx) = mpsc::channel::<XStream>(1);
    let (client_stream_tx, mut client_stream_rx) = mpsc::channel::<XStream>(1);

    // Channel for requesting stream opening
    let (stream_req_tx, mut stream_req_rx) =
        mpsc::channel::<(PeerId, oneshot::Sender<Result<XStream, String>>)>(1);

    // Create shutdown channels for both server and client tasks
    let (server_shutdown_tx, mut server_shutdown_rx) = mpsc::channel::<()>(1);
    let (client_shutdown_tx, mut client_shutdown_rx) = mpsc::channel::<()>(1);

    // Make server listen on QUIC
    let server_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    server_swarm.listen_on(server_addr.clone()).expect("Failed to listen on QUIC");
    println!("QUIC Server listening on: {}", server_addr);

    // Get actual listen address
    let listen_addr = wait_for_quic_listen_addr(&mut server_swarm).await;
    println!("QUIC Server actually listening on: {}", listen_addr);

    // Connection flags
    let server_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let client_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let server_connected_clone = server_connected.clone();

    // Spawn server task
    tokio::spawn(async move {
        println!("QUIC Server task started");

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = server_shutdown_rx.recv() => {
                    println!("QUIC Server received shutdown signal, exiting");
                    break;
                },

                // Process swarm events
                event = server_swarm.next() => {
                    match event {
                        Some(SwarmEvent::ConnectionEstablished { peer_id, .. }) => {
                            println!("QUIC Server: Connection established with {}", peer_id);
                            if peer_id == client_peer_id {
                                server_connected_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                        },
                        Some(SwarmEvent::Behaviour(XStreamEvent::IncomingStream { stream })) => {
                            println!("QUIC Server: Received incoming stream from {} with ID {:?}", stream.peer_id, stream.id);
                            server_stream_tx.send(stream).await
                                .expect("Failed to send QUIC server stream to test");
                            println!("QUIC Server: Successfully sent stream to test");
                        },
                        Some(event) => {
                            println!("QUIC Server event: {:?}", event);
                        },
                        None => break,
                    }
                }
            }
        }

        println!("QUIC Server task exiting");
    });

    let client_connected_clone = client_connected.clone();

    // Spawn client task
    tokio::spawn(async move {
        println!("QUIC Client task started");

        // Connect to server
        println!("QUIC Client connecting to server at {}", listen_addr);
        client_swarm.dial(DialOpts::peer_id(server_peer_id).addresses(vec![listen_addr.clone()]).build()).unwrap();

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = client_shutdown_rx.recv() => {
                    println!("QUIC Client received shutdown signal, exiting");
                    break;
                },

                // Process swarm events
                event = client_swarm.next() => {
                    match event {
                        Some(SwarmEvent::ConnectionEstablished { peer_id, .. }) => {
                            println!("QUIC Client: Connection established with {}", peer_id);
                            if peer_id == server_peer_id {
                                client_connected_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                        },
                        Some(SwarmEvent::Behaviour(XStreamEvent::StreamEstablished { peer_id, stream_id })) => {
                            println!("QUIC Client: Stream established with {} (ID: {:?})", peer_id, stream_id);
                        },
                        Some(event) => {
                            println!("QUIC Client event: {:?}", event);
                        },
                        None => {
                            println!("QUIC Client swarm returned None, exiting");
                            break;
                        }
                    }
                },

                // Process stream opening requests
                request = stream_req_rx.recv() => {
                    match request {
                        Some((peer_id, response_channel)) => {
                            println!("QUIC Client: Received request to open stream to {}", peer_id);
                            client_swarm.behaviour_mut().open_stream(peer_id, response_channel).await;
                            println!("QUIC Client: Stream open request sent");
                        },
                        None => {
                            // Create a new channel that will never be consumed, just to keep select! happy
                            let (_, new_rx) = mpsc::channel::<(PeerId, oneshot::Sender<Result<XStream, String>>)>(1);
                            stream_req_rx = new_rx;
                            println!("QUIC Client: Stream request channel closed, continuing to run");
                        }
                    }
                }
            }
        }

        println!("QUIC Client task exiting");
    });

    // Wait for connections to be established
    println!("Waiting for QUIC connections to be established...");
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
            "QUIC Server did not connect to client after {} attempts",
            MAX_ATTEMPTS
        );
    }
    if !client_connected.load(std::sync::atomic::Ordering::SeqCst) {
        panic!(
            "QUIC Client did not connect to server after {} attempts",
            MAX_ATTEMPTS
        );
    }

    println!("QUIC Both sides connected successfully!");

    // Wait a moment to ensure connection stability
    sleep(Duration::from_millis(300)).await;

    // Request to open a stream
    println!("Requesting QUIC client to open stream to server");
    let (tx, rx) = oneshot::channel();

    stream_req_tx
        .send((server_peer_id, tx))
        .await
        .expect("Failed to send QUIC stream open request");
    println!("QUIC Stream open request sent to client task");

    // Wait for the stream from the oneshot channel
    println!("Waiting for QUIC client stream from oneshot channel");
    let client_stream = match timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(stream))) => {
            println!("Got QUIC client stream from oneshot with ID {:?}!", stream.id);
            stream
        }
        Ok(Ok(Err(e))) => panic!("Error opening QUIC stream: {}", e),
        Ok(Err(_)) => panic!("QUIC Oneshot channel closed unexpectedly"),
        Err(_) => panic!("Timeout waiting for QUIC oneshot response after 5 seconds"),
    };

    // Forward it to the test
    client_stream_tx
        .send(client_stream)
        .await
        .expect("Failed to forward QUIC client stream to test");

    // Wait for server stream
    println!("Waiting for QUIC server stream...");
    let server_stream = timeout(Duration::from_secs(5), server_stream_rx.recv())
        .await
        .expect("Timeout waiting for QUIC server stream after 5 seconds")
        .expect("QUIC Server stream channel closed unexpectedly");
    println!("Got QUIC server stream with ID {:?}!", server_stream.id);

    // Get client stream from channel
    println!("Getting QUIC client stream from channel...");
    let client_stream = timeout(Duration::from_secs(1), client_stream_rx.recv())
        .await
        .expect("Timeout waiting for QUIC client stream from channel")
        .expect("QUIC Client stream channel closed unexpectedly");
    println!("Got QUIC client stream from channel!");

    // Create a ShutdownManager to handle proper shutdown coordination
    let shutdown_manager = QuicShutdownManager {
        client_shutdown: client_shutdown_tx,
        server_shutdown: server_shutdown_tx,
    };

    // Return the test pair and shutdown manager
    (
        XStreamQuicTestPair {
            client_stream,
            server_stream,
            client_peer_id,
            server_peer_id,
        },
        shutdown_manager,
    )
}

/// Создает узел с QUIC транспортом
async fn create_quic_swarm() -> (Swarm<XStreamNetworkBehaviour>, PeerId) {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();
    
    // Создаем QUIC транспорт
    let quic_config = quic::Config::new(&keypair);
    let quic_transport = quic::tokio::Transport::new(quic_config);
    
    // Создаем swarm с XStream поведением
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|_key| quic_transport)
        .expect("Не удалось создать QUIC транспорт")
        .with_behaviour(|_key| XStreamNetworkBehaviour::new())
        .expect("Не удалось создать XStream поведение")
        .build();
    
    (swarm, peer_id)
}

/// Ожидает адрес прослушивания от QUIC swarm
async fn wait_for_quic_listen_addr(swarm: &mut Swarm<XStreamNetworkBehaviour>) -> Multiaddr {
    timeout(Duration::from_secs(2), async {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                return address;
            }
        }
    })
    .await
    .expect("Таймаут ожидания QUIC адреса прослушивания")
}

/// Test close() functionality with QUIC transport
/// Проверяет базовую функциональность close() с QUIC
/// 
/// ВАЖНО: Используем close() вместо close_read() из-за гонок в QUIC
/// - close() атомарно закрывает обе половины потока (read и write)
/// - Уведомляет транспортный уровень о полном закрытии
/// - Задержка необходима для распространения состояния в QUIC event loop
/// - В реальной сети QUIC требуется время для обработки закрытия
#[tokio::test]
async fn test_quic_close_functionality() {
    let (mut test_pair, shutdown_manager) = create_xstream_quic_test_pair().await;
    let test_data = b"Test data for QUIC close".to_vec();
    
    // Write some data first
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // ВАЖНО: Используем close() вместо close_read() для атомарного закрытия
    // close() гарантирует корректное уведомление транспортного уровня
    // и предотвращает гонки в QUIC
    test_pair.client_stream.close().await.unwrap();

    // Задержка для распространения состояния закрытия в QUIC
    // QUIC требует времени для обработки закрытия в event loop
    sleep(Duration::from_millis(1)).await;

    // Client should not be able to read after close()
    let client_read_result = test_pair.client_stream.read().await;
    assert!(client_read_result.is_err(), "QUIC Client should not be able to read after close()");

    // Client should not be able to write after close()
    let client_write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    assert!(client_write_result.is_err(), "QUIC Client should not be able to write after close()");

    // В QUIC после close() сервер не может писать - соединение полностью закрывается
    // Это отличается от частичного закрытия close_read(), где сервер мог продолжать писать
    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    assert!(server_write_result.is_err(), "QUIC Server should NOT be able to write after client close() - QUIC fully closes the connection");

    let server_flush_result = test_pair.server_stream.flush().await;
    assert!(server_flush_result.is_err(), "QUIC Server should NOT be able to flush after client close() - QUIC fully closes the connection");

    // Close both streams
    test_pair.server_stream.close().await.unwrap();
    test_pair.client_stream.close().await.unwrap();

    shutdown_manager.shutdown().await;
}

/// Test that close() affects all clones with QUIC
/// Проверяет, что close() влияет на все клоны XStream с QUIC
/// 
/// ВАЖНО: Используем close() вместо close_read() для атомарного закрытия
/// - close() гарантирует корректное уведомление транспортного уровня
/// - Все клоны разделяют одно состояние закрытия
/// - Предотвращает гонки в QUIC при частичном закрытии
#[tokio::test]
async fn test_quic_close_affects_clones() {
    let (mut test_pair, shutdown_manager) = create_xstream_quic_test_pair().await;
    let test_data = b"Test data for QUIC clone close".to_vec();

    // Create clones
    let client_clone1 = test_pair.client_stream.clone();
    let client_clone2 = test_pair.client_stream.clone();

    // Write some data first
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // ВАЖНО: Используем close() вместо close_read() для атомарного закрытия
    // close() гарантирует корректное уведомление транспортного уровня
    // и предотвращает гонки в QUIC
    test_pair.client_stream.close().await.unwrap();

    // Задержка для распространения состояния закрытия в QUIC
    sleep(Duration::from_millis(1)).await;

    // All clones should also be unable to read after close()
    let clone1_read_result = client_clone1.read().await;
    assert!(clone1_read_result.is_err(), "QUIC Clone 1 should not be able to read after close()");

    let clone2_read_result = client_clone2.read().await;
    assert!(clone2_read_result.is_err(), "QUIC Clone 2 should not be able to read after close()");

    // All clones should also be unable to write after close()
    let clone1_write_result = client_clone1.write_all(test_data.clone()).await;
    assert!(clone1_write_result.is_err(), "QUIC Clone 1 should not be able to write after close()");

    let clone2_write_result = client_clone2.write_all(test_data.clone()).await;
    assert!(clone2_write_result.is_err(), "QUIC Clone 2 should not be able to write after close()");

    // Original should also be unable to read and write
    let original_read_result = test_pair.client_stream.read().await;
    assert!(original_read_result.is_err(), "QUIC Original should not be able to read after close()");

    let original_write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    assert!(original_write_result.is_err(), "QUIC Original should not be able to write after close()");

    // В QUIC после close() сервер не может писать - соединение полностью закрывается
    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    assert!(server_write_result.is_err(), "QUIC Server should NOT be able to write after client close() - QUIC fully closes the connection");

    // Close both streams
    test_pair.server_stream.close().await.unwrap();
    test_pair.client_stream.close().await.unwrap();

    shutdown_manager.shutdown().await;
}

/// Test server operations after client close with QUIC
/// Проверяет операции сервера после закрытия клиента с QUIC
/// 
/// ВАЖНО: Используем close() вместо частичного закрытия из-за гонок в QUIC
/// - close() атомарно закрывает обе половины потока
/// - Уведомляет транспортный уровень о полном закрытии
/// - Предотвращает гонки при частичном закрытии в QUIC
#[tokio::test]
async fn test_quic_server_operations_after_client_close() {
    let (mut test_pair, shutdown_manager) = create_xstream_quic_test_pair().await;
    let test_data = b"Test data for QUIC server operations".to_vec();

    // Write some data first
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // ВАЖНО: Используем close() вместо close_write() и close_read() для атомарного закрытия
    // close() гарантирует корректное уведомление транспортного уровня
    // и предотвращает гонки в QUIC
    test_pair.client_stream.close().await;

    // Задержка для распространения состояния закрытия в QUIC
    sleep(Duration::from_millis(1)).await;

    // Client should not be able to write after close()
    let client_write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    assert!(client_write_result.is_err(), "QUIC Client should not be able to write after close()");

    // Client should not be able to read after close()
    let client_read_result = test_pair.client_stream.read().await;
    assert!(client_read_result.is_err(), "QUIC Client should not be able to read after close()");

    // Test server operations after client close
    // В QUIC после close() сервер не может писать - соединение полностью закрывается
    // Это отличается от частичного закрытия close_read(), где сервер мог продолжать писать
    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    assert!(server_write_result.is_err(), "QUIC Server should NOT be able to write after client close() - QUIC fully closes the connection");

    let server_flush_result = test_pair.server_stream.flush().await;
    assert!(server_flush_result.is_err(), "QUIC Server should NOT be able to flush after client close() - QUIC fully closes the connection");

    // Но клиент не должен быть способен читать данные после close()
    let client_read_after_close = test_pair.client_stream.read_exact(test_data.len()).await;
    assert!(client_read_after_close.is_err(), "QUIC Client should not be able to read after close()");

    // Закрываем серверный поток для полной очистки
    test_pair.server_stream.close().await.unwrap();

    shutdown_manager.shutdown().await;
}

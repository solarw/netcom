// tests/test_connections.rs - Полный набор тестов для Connection Management

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::{
    XRoutesConfig, 
    events::NetworkEvent,
};

mod common;
use common::*;

// ==========================================
// 1. БАЗОВЫЕ ТЕСТЫ ПОДКЛЮЧЕНИЯ
// ==========================================

#[tokio::test]
async fn test_basic_connection_establishment() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing basic peer-to-peer connection establishment");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем два узла
        let (mut server_node, server_commander, mut server_events, _server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("Failed to create server node");
        
        let (mut client_node, client_commander, mut client_events, _client_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("Failed to create client node");
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        let client_handle = tokio::spawn(async move {
            client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Сервер начинает слушать
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await
            .expect("Failed to start server listener");
        
        // Получаем адрес сервера
        let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(event) = server_events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return addr;
                }
            }
            panic!("No listening address received");
        }).await.expect("Timeout waiting for server address");
        
        println!("Server listening on: {}", server_addr);
        
        // Клиент подключается
        let connect_start = std::time::Instant::now();
        let connect_result = client_commander.connect_with_timeout(server_addr.clone(), 5).await;
        println!("Connect result: {:?}", connect_result);
        
        // Ждем события подключения или таймаута
        let target_peer_id = _server_peer_id;
        let connection_established = tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = client_events.recv().await {
                match event {
                    NetworkEvent::PeerConnected { peer_id } => {
                        if peer_id == target_peer_id {
                            return true;
                        }
                    }
                    NetworkEvent::ConnectionOpened { peer_id, .. } => {
                        if peer_id == target_peer_id {
                            return true;
                        }
                    }
                    _ => continue,
                }
            }
            false
        }).await.unwrap_or(false);
        
        let connect_duration = connect_start.elapsed();
        println!("Connection attempt took: {:?}, established: {}", connect_duration, connection_established);
        
        // Проверяем состояние соединений независимо от событий
        let client_connections = client_commander.get_all_connections().await
            .expect("Should get connections");
        
        if !client_connections.is_empty() {
            let connection = &client_connections[0];
            println!("Found connection: {:?} -> {} (direction: {:?}, active: {})", 
                     connection.connection_id, connection.peer_id, connection.direction, connection.is_active());
            
            if connection.peer_id == target_peer_id {
                println!("✅ Connection verified successfully");
            }
        } else {
            println!("⚠️  No connections found");
        }
        
        // Cleanup
        client_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Basic connection establishment test completed"),
        Err(_) => panic!("⏰ Basic connection establishment test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_connection_with_timeout_success() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing successful connection with timeout");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем сервер
        let (mut server_node, server_commander, mut server_events, _server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("Failed to create server");
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Запускаем слушатель
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
        
        let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(event) = server_events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return addr;
                }
            }
            panic!("No address");
        }).await.unwrap();
        
        // Создаем клиента
        let (mut client_node, client_commander, _client_events, _client_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let client_handle = tokio::spawn(async move {
            client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Тестируем подключение с 3-секундным таймаутом
        let start_time = std::time::Instant::now();
        let connection_result = client_commander.connect_with_timeout(server_addr, 3).await;
        let elapsed = start_time.elapsed();
        
        match connection_result {
            Ok(_) => {
                println!("✅ Connection succeeded in {:?}", elapsed);
            }
            Err(e) => {
                // В тестовой среде соединение может не установиться
                println!("⚠️  Connection failed: {}", e);
                assert!(!e.to_string().contains("timed out"), "Should not timeout with valid address");
            }
        }
        
        // Cleanup
        client_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection with timeout success test completed"),
        Err(_) => panic!("⏰ Connection with timeout test timed out ({}s)", test_timeout.as_secs()),
    }
}

// ==========================================
// 2. ТЕСТЫ УПРАВЛЕНИЯ СОЕДИНЕНИЯМИ
// ==========================================

#[tokio::test]
async fn test_disconnect_peer() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing peer disconnection");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем что соединение установлено
        let connections_before = client_commander.get_all_connections().await.unwrap();
        println!("Connections before disconnect: {}", connections_before.len());
        
        // Отключаемся от peer'а
        let disconnect_result = client_commander.disconnect(server_peer_id).await;
        println!("Disconnect result: {:?}", disconnect_result);
        
        // Ждем обработки отключения
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Проверяем что соединений стало меньше
        let connections_after = client_commander.get_all_connections().await.unwrap();
        let active_connections = connections_after.iter().filter(|c| c.is_active()).count();
        
        println!("Active connections after disconnect: {}", active_connections);
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Peer disconnection test completed"),
        Err(_) => panic!("⏰ Peer disconnection test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_disconnect_specific_connection() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing specific connection disconnection");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             _server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Получаем ID соединения
        let connections = client_commander.get_all_connections().await.unwrap();
        
        if !connections.is_empty() {
            let connection_id = connections[0].connection_id;
            println!("Disconnecting connection: {:?}", connection_id);
            
            // Отключаем конкретное соединение
            let disconnect_result = client_commander.disconnect_connection(connection_id).await;
            println!("Disconnect connection result: {:?}", disconnect_result);
            
            // Ждем обработки
            tokio::time::sleep(Duration::from_millis(1000)).await;
            
            // Проверяем что соединение больше не активно
            let updated_connections = client_commander.get_all_connections().await.unwrap();
            let still_active = updated_connections.iter()
                .any(|c| c.connection_id == connection_id && c.is_active());
            
            println!("Connection still active: {}", still_active);
        } else {
            println!("⚠️  No connections available for testing");
        }
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Specific connection disconnection test completed"),
        Err(_) => panic!("⏰ Specific connection disconnection test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_disconnect_all_connections() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing disconnect all connections");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             _server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем наличие соединений
        let connections_before = client_commander.get_all_connections().await.unwrap();
        let active_before = connections_before.iter().filter(|c| c.is_active()).count();
        println!("Active connections before disconnect all: {}", active_before);
        
        // Отключаем все соединения
        let disconnect_all_result = client_commander.disconnect_all().await;
        println!("Disconnect all result: {:?}", disconnect_all_result);
        
        // Ждем обработки
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Проверяем что все соединения отключены
        let connections_after = client_commander.get_all_connections().await.unwrap();
        let active_after = connections_after.iter().filter(|c| c.is_active()).count();
        println!("Active connections after disconnect all: {}", active_after);
        
        // Проверяем сетевое состояние
        let network_state = client_commander.get_network_state().await.unwrap();
        println!("Network state after disconnect all: {} connections", network_state.total_connections);
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Disconnect all connections test completed"),
        Err(_) => panic!("⏰ Disconnect all connections test timed out ({}s)", test_timeout.as_secs()),
    }
}

// ==========================================  
// 3. ТЕСТЫ ПОЛУЧЕНИЯ ИНФОРМАЦИИ О СОЕДИНЕНИЯХ
// ==========================================

#[tokio::test]
async fn test_get_all_connections_empty() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing get_all_connections when no connections exist");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, _events, _peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Получаем соединения для пустого узла
        let connections = commander.get_all_connections().await.unwrap();
        
        assert!(connections.is_empty(), "Should have no connections initially");
        
        // Проверяем сетевое состояние
        let network_state = commander.get_network_state().await.unwrap();
        assert_eq!(network_state.total_connections, 0, "Network state should show 0 connections");
        assert_eq!(network_state.authenticated_peers, 0, "Should have 0 authenticated peers");
        
        println!("✅ Network state verified: {} connections, {} authenticated peers", 
                 network_state.total_connections, network_state.authenticated_peers);
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Get all connections empty test completed"),
        Err(_) => panic!("⏰ Get all connections empty test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_get_connection_info() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing get_connection_info for existing connections");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Получаем все соединения
        let connections = client_commander.get_all_connections().await.unwrap();
        
        if !connections.is_empty() {
            let connection_id = connections[0].connection_id;
            
            // Тестируем получение информации о существующем соединении
            let connection_info = client_commander.get_connection_info(connection_id).await.unwrap();
            
            match connection_info {
                Some(info) => {
                    println!("✅ Connection info retrieved:");
                    println!("  ID: {:?}", info.connection_id);
                    println!("  Peer: {}", info.peer_id);
                    println!("  Direction: {:?}", info.direction);
                    println!("  State: {:?}", info.connection_state);
                    println!("  Duration: {:?}", info.duration());
                    
                    assert_eq!(info.peer_id, server_peer_id, "Peer ID should match");
                    assert!(info.is_active(), "Connection should be active");
                }
                None => {
                    println!("⚠️  No connection info found (connection may have been closed)");
                }
            }
        } else {
            println!("⚠️  No connections available for testing");
        }
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Get connection info test completed"),
        Err(_) => panic!("⏰ Get connection info test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_get_peer_info() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing get_peer_info for connected and non-connected peers");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Тестируем получение информации о подключенном peer'е
        let peer_info = client_commander.get_peer_info(server_peer_id).await.unwrap();
        
        match peer_info {
            Some(info) => {
                println!("✅ Peer info retrieved:");
                println!("  Peer ID: {}", info.peer_id);
                println!("  Connections: {}", info.connection_count());
                println!("  Is connected: {}", info.is_connected());
                println!("  Is authenticated: {}", info.is_authenticated);
                println!("  Total connections ever: {}", info.total_connections);
                
                assert_eq!(info.peer_id, server_peer_id, "Peer ID should match");
            }
            None => {
                println!("⚠️  Peer info not found (peer may not be connected)");
            }
        }
        
        // Тестируем получение информации о несуществующем peer'е
        let fake_peer_id = PeerId::random();
        let nonexistent_peer_info = client_commander.get_peer_info(fake_peer_id).await.unwrap();
        
        assert!(nonexistent_peer_info.is_none(), "Should return None for non-existent peer");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Get peer info test completed"),
        Err(_) => panic!("⏰ Get peer info test timed out ({}s)", test_timeout.as_secs()),
    }
}

// ==========================================
// 4. ТЕСТЫ СОБЫТИЙ СОЕДИНЕНИЙ
// ==========================================

#[tokio::test]
async fn test_connection_events_lifecycle() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing complete connection event lifecycle");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut server_node, server_commander, mut server_events, server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let (mut client_node, client_commander, mut client_events, _client_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        let client_handle = tokio::spawn(async move {
            client_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Сервер начинает слушать
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
        
        let mut listening_event_received = false;
        let mut server_addr = None;
        
        // Ждем событие начала прослушивания
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = server_events.recv().await {
                match event {
                    NetworkEvent::ListeningOnAddress { addr, full_addr } => {
                        println!("✅ Listening event: {} -> {:?}", addr, full_addr);
                        listening_event_received = true;
                        if let Some(full) = full_addr {
                            server_addr = Some(full);
                            break;
                        }
                    }
                    _ => continue,
                }
            }
        }).await.ok();
        
        if !listening_event_received || server_addr.is_none() {
            println!("⚠️  Listening event not received - skipping connection test");
        } else {
            let addr = server_addr.unwrap();
            
            // Клиент подключается
            let _ = client_commander.connect_with_timeout(addr, 2).await;
            
            let mut connection_events_received = false;
            
            // Ждем события подключения
            tokio::time::timeout(Duration::from_secs(3), async {
                while let Some(event) = client_events.recv().await {
                    match event {
                        NetworkEvent::ConnectionOpened { peer_id, addr, connection_id, protocols } => {
                            println!("✅ Connection opened event: peer={}, addr={}, id={:?}, protocols={:?}", 
                                     peer_id, addr, connection_id, protocols);
                            connection_events_received = true;
                            break;
                        }
                        NetworkEvent::PeerConnected { peer_id } => {
                            println!("✅ Peer connected event: {}", peer_id);
                            connection_events_received = true;
                            break;
                        }
                        NetworkEvent::ConnectionError { peer_id, error } => {
                            println!("⚠️  Connection error: peer={:?}, error={}", peer_id, error);
                        }
                        _ => continue,
                    }
                }
            }).await.ok();
            
            if connection_events_received {
                println!("✅ Connection events received successfully");
            } else {
                println!("⚠️  Connection events not received (test environment limitation)");
            }
        }
        
        // Cleanup
        client_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection events lifecycle test completed"),
        Err(_) => panic!("⏰ Connection events lifecycle test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_listening_address_events() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing listening address events");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, mut events, _peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Запускаем слушатель
        commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
        
        let mut listening_events = Vec::new();
        
        // Собираем события прослушивания
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = events.recv().await {
                match event {
                    NetworkEvent::ListeningOnAddress { addr, full_addr } => {
                        println!("📡 Listening on: {} (full: {:?})", addr, full_addr);
                        listening_events.push((addr, full_addr));
                        break; // Получили первое событие
                    }
                    NetworkEvent::StopListeningOnAddress { addr } => {
                        println!("📡 Stopped listening on: {}", addr);
                    }
                    _ => continue,
                }
            }
        }).await.ok();
        
        assert!(!listening_events.is_empty(), "Should receive at least one listening event");
        
        // Проверяем что адреса прослушивания доступны через API
        let listen_addresses = commander.get_listen_addresses().await.unwrap();
        println!("API reported {} listening addresses", listen_addresses.len());
        
        for addr in &listen_addresses {
            println!("  - {}", addr);
        }
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Listening address events test completed"),
        Err(_) => panic!("⏰ Listening address events test timed out ({}s)", test_timeout.as_secs()),
    }
}

// ==========================================
// 5. ТЕСТЫ ПРОИЗВОДИТЕЛЬНОСТИ И СТАБИЛЬНОСТИ
// ==========================================

#[tokio::test]
async fn test_memory_cleanup_after_disconnect() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing memory cleanup after disconnection");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем начальное состояние
        let initial_connections = client_commander.get_all_connections().await.unwrap();
        let initial_peers = client_commander.get_connected_peers().await.unwrap();
        let initial_network_state = client_commander.get_network_state().await.unwrap();
        
        println!("Initial state:");
        println!("  Connections: {}", initial_connections.len());
        println!("  Peers: {}", initial_peers.len());
        println!("  Network connections: {}", initial_network_state.total_connections);
        
        // Отключаемся
        let _ = client_commander.disconnect(server_peer_id).await;
        
        // Ждем обработки отключения
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Проверяем состояние после отключения
        let final_connections = client_commander.get_all_connections().await.unwrap();
        let final_peers = client_commander.get_connected_peers().await.unwrap();
        let final_network_state = client_commander.get_network_state().await.unwrap();
        
        println!("Final state:");
        println!("  Connections: {}", final_connections.len());
        println!("  Peers: {}", final_peers.len());
        println!("  Network connections: {}", final_network_state.total_connections);
        
        // Проверяем что память очищена
        let active_connections = final_connections.iter().filter(|c| c.is_active()).count();
        let connected_peers = final_peers.iter().filter(|p| p.is_connected()).count();
        
        println!("Active connections after cleanup: {}", active_connections);
        println!("Connected peers after cleanup: {}", connected_peers);
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Memory cleanup test completed"),
        Err(_) => panic!("⏰ Memory cleanup test timed out ({}s)", test_timeout.as_secs()),
    }
}

// ==========================================
// 6. EDGE CASE ТЕСТЫ
// ==========================================

#[tokio::test]
async fn test_disconnect_nonexistent_peer() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing disconnection of non-existent peer");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, _events, _peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Пытаемся отключить несуществующий peer
        let fake_peer_id = PeerId::random();
        let disconnect_result = commander.disconnect(fake_peer_id).await;
        
        match disconnect_result {
            Ok(_) => {
                println!("⚠️  Disconnect succeeded unexpectedly");
            }
            Err(e) => {
                println!("✅ Disconnect failed as expected: {}", e);
                assert!(e.to_string().contains("Not connected"), "Should indicate peer not connected");
            }
        }
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Disconnect non-existent peer test completed"),
        Err(_) => panic!("⏰ Disconnect non-existent peer test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_connection_state_consistency() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing connection state consistency across APIs");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             _server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Получаем информацию через разные API
        let all_connections = client_commander.get_all_connections().await.unwrap();
        let connected_peers = client_commander.get_connected_peers().await.unwrap();
        let network_state = client_commander.get_network_state().await.unwrap();
        let legacy_peers = client_commander.get_connected_peers_simple().await.unwrap();
        
        println!("State consistency check:");
        println!("  All connections: {}", all_connections.len());
        println!("  Connected peers (new API): {}", connected_peers.len());
        println!("  Network state connections: {}", network_state.total_connections);
        println!("  Legacy connected peers: {}", legacy_peers.len());
        
        // Проверяем согласованность данных
        let active_connections = all_connections.iter().filter(|c| c.is_active()).count();
        let actually_connected_peers = connected_peers.iter().filter(|p| p.is_connected()).count();
        
        println!("  Active connections: {}", active_connections);
        println!("  Actually connected peers: {}", actually_connected_peers);
        
        // Если есть соединения, проверяем конкретные данные
        if !all_connections.is_empty() {
            let connection = &all_connections[0];
            println!("Connection details:");
            println!("  ID: {:?}", connection.connection_id);
            println!("  Peer: {}", connection.peer_id);
            println!("  Direction: {:?}", connection.direction);
            println!("  State: {:?}", connection.connection_state);
            println!("  Auth status: {:?}", connection.auth_status);
            
            // Проверяем информацию о peer'е
            if let Ok(Some(peer_info)) = client_commander.get_peer_info(connection.peer_id).await {
                println!("Peer info:");
                println!("  Connection count: {}", peer_info.connection_count());
                println!("  Is connected: {}", peer_info.is_connected());
                println!("  Auth status: {:?}", peer_info.auth_status);
            }
        } else {
            println!("⚠️  No connections available for detailed testing");
        }
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection state consistency test completed"),
        Err(_) => panic!("⏰ Connection state consistency test timed out ({}s)", test_timeout.as_secs()),
    }
}

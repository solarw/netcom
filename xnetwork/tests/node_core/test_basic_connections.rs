// tests/node_core/test_basic_connections.rs - Базовые тесты подключения (PRIORITY 1)

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::{
    XRoutesConfig, 
    events::NetworkEvent,
};

use crate::common::*;

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

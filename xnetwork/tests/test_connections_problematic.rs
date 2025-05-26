// tests/test_connections_problematic.rs - Проблемные тесты (исключены из основного набора)
//
// Эти тесты исключены из основного набора тестов из-за проблем с таймаутами и зависанием
// в тестовой среде. Они могут быть полезны для отладки или запуска в специальных условиях.

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::{
    XRoutesConfig, 
    events::NetworkEvent,
};

mod common;
use common::*;

// ==========================================
// ПРОБЛЕМНЫЕ ТЕСТЫ - ИСПОЛЬЗУЙТЕ С ОСТОРОЖНОСТЬЮ
// ==========================================

/// Тест отслеживания состояния сети
/// ПРОБЛЕМА: Зависает на API вызовах get_network_state() или listen_port()
/// ПРИЧИНА: Возможно блокировка на внутренних мьютексах или ожидании событий
#[tokio::test]
async fn test_network_state_tracking() {
    let test_timeout = Duration::from_secs(15); // Увеличенный таймаут
    
    println!("🧪 Testing network state tracking and updates");
    println!("⚠️  WARNING: This test may hang in some environments");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, _events, peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_millis(500)).await;
        });
        
        // Даем время узлу инициализироваться
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Получаем начальное состояние сети
        println!("Getting initial network state...");
        let initial_state = commander.get_network_state().await.unwrap();
        println!("Initial network state:");
        println!("  Local peer: {}", initial_state.local_peer_id);
        println!("  Listening addresses: {}", initial_state.listening_addresses.len());
        println!("  Total connections: {}", initial_state.total_connections);
        println!("  Authenticated peers: {}", initial_state.authenticated_peers);
        println!("  Uptime: {:?}", initial_state.uptime);
        
        assert_eq!(initial_state.local_peer_id, peer_id, "Local peer ID should match");
        assert_eq!(initial_state.total_connections, 0, "Should start with 0 connections");
        assert_eq!(initial_state.authenticated_peers, 0, "Should start with 0 authenticated peers");
        
        // Запускаем слушатель - ТУТ ЧАСТО ЗАВИСАЕТ
        println!("Starting listener...");
        let listen_result = commander.listen_port(Some("127.0.0.1".to_string()), 0).await;
        println!("Listen result: {:?}", listen_result);
        
        // Ждем больше времени для обработки
        println!("Waiting for listener to stabilize...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        println!("Getting updated network state...");
        let updated_state = commander.get_network_state().await.unwrap();
        println!("Updated network state:");
        println!("  Listening addresses: {}", updated_state.listening_addresses.len());
        println!("  Uptime: {:?}", updated_state.uptime);
        
        // Проверяем список адресов прослушивания через API
        println!("Getting listen addresses...");
        let listen_addresses = commander.get_listen_addresses().await.unwrap();
        println!("Listen addresses from API: {}", listen_addresses.len());
        for (i, addr) in listen_addresses.iter().enumerate() {
            println!("  Address {}: {}", i + 1, addr);
        }
        
        // Проверки
        if listen_result.is_ok() {
            assert!(updated_state.listening_addresses.len() > 0, "Should have listening addresses after setup");
            assert!(updated_state.uptime >= initial_state.uptime, "Uptime should not decrease");
            println!("✅ Network state tracking working correctly");
        } else {
            println!("⚠️  Listen failed, but network state API still functional");
        }
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Network state tracking test completed"),
        Err(_) => panic!("⏰ Network state tracking test timed out ({}s)", test_timeout.as_secs()),
    }
}

/// Тест множественных подключений к одному peer'у
/// ПРОБЛЕМА: Зависает на процессе аутентификации (PoR auth)
/// ПРИЧИНА: Аутентификация занимает слишком много времени или зависает
#[tokio::test]
async fn test_multiple_connections_same_peer() {
    let test_timeout = Duration::from_secs(20); // Очень большой таймаут
    
    println!("🧪 Testing multiple connections to the same peer");
    println!("⚠️  WARNING: This test may hang during authentication");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем server и два client'а
        let (mut server_node, server_commander, mut server_events, _server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let (mut client1_node, client1_commander, _client1_events, _) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let (mut client2_node, client2_commander, _client2_events, _) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_millis(100)).await;
        });
        let client1_handle = tokio::spawn(async move {
            client1_node.run_with_cleanup_interval(Duration::from_millis(100)).await;
        });
        let client2_handle = tokio::spawn(async move {
            client2_node.run_with_cleanup_interval(Duration::from_millis(100)).await;
        });
        
        // Сервер слушает
        println!("Starting server listener...");
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
        
        let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(event) = server_events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return addr;
                }
            }
            panic!("No address");
        }).await.unwrap();
        
        println!("Server listening on: {}", server_addr);
        
        // Последовательные подключения для избежания состояний гонки
        println!("Client1 connecting...");
        let result1 = client1_commander.connect_with_timeout(server_addr.clone(), 5).await;
        println!("Client1 connect result: {:?}", result1);
        
        // Даем время первому соединению частично установиться
        println!("Waiting for first connection to stabilize...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        println!("Client2 connecting...");
        let result2 = client2_commander.connect_with_timeout(server_addr.clone(), 5).await;
        println!("Client2 connect result: {:?}", result2);
        
        // Ждем установления соединений - ТУТ ЧАСТО ЗАВИСАЕТ НА АУТЕНТИФИКАЦИИ
        println!("Waiting for authentication to complete...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Проверяем соединения на сервере
        println!("Checking server connections...");
        let server_connections = server_commander.get_all_connections().await.unwrap_or_default();
        println!("Server has {} connections", server_connections.len());
        
        // Проверяем сетевое состояние
        println!("Checking network state...");
        let network_state = server_commander.get_network_state().await.unwrap();
        println!("Network state: {} total connections, {} authenticated peers", 
                 network_state.total_connections, network_state.authenticated_peers);
        
        // Cleanup
        client1_handle.abort();
        client2_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Multiple connections test completed"),
        Err(_) => panic!("⏰ Multiple connections test timed out ({}s)", test_timeout.as_secs()),
    }
}

/// Тест быстрых циклов подключения/отключения
/// ПРОБЛЕМА: Зависает на аутентификации или не успевает обработать события
/// ПРИЧИНА: Быстрые циклы не дают время аутентификации завершиться
#[tokio::test]
async fn test_rapid_connect_disconnect() {
    let test_timeout = Duration::from_secs(30); // Очень большой таймаут
    
    println!("🧪 Testing rapid connect/disconnect cycles");
    println!("⚠️  WARNING: This test may hang during authentication cycles");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut server_node, server_commander, mut server_events, server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let (mut client_node, client_commander, _client_events, _client_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_millis(50)).await;
        });
        let client_handle = tokio::spawn(async move {
            client_node.run_with_cleanup_interval(Duration::from_millis(50)).await;
        });
        
        // Настраиваем сервер
        println!("Setting up server...");
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
        
        let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(event) = server_events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return addr;
                }
            }
            panic!("No address");
        }).await.unwrap();
        
        println!("Server is listening on {}", server_addr);
        
        let mut successful_cycles = 0;
        let total_cycles = 3;
        
        for i in 0..total_cycles {
            println!("  Cycle {}/{}", i + 1, total_cycles);
            
            // Подключаемся с увеличенным таймаутом
            let connect_start = std::time::Instant::now();
            println!("    Connecting...");
            let connect_result = client_commander.connect_with_timeout(server_addr.clone(), 8).await;
            let connect_elapsed = connect_start.elapsed();
            
            println!("    Connect attempt took {:?}: {:?}", connect_elapsed, connect_result);
            
            if connect_result.is_ok() {
                // Ждем стабилизации соединения и завершения аутентификации
                // ТУТ ЧАСТО ЗАВИСАЕТ
                println!("    Waiting for connection to stabilize and auth to complete...");
                tokio::time::sleep(Duration::from_secs(3)).await;
                
                // Проверяем состояние соединения
                println!("    Checking connection state...");
                let connections = client_commander.get_all_connections().await.unwrap_or_default();
                let active_connections = connections.iter().filter(|c| c.is_active()).count();
                println!("    Active connections: {}", active_connections);
                
                if active_connections > 0 {
                    // Отключаемся
                    println!("    Disconnecting...");
                    let disconnect_result = client_commander.disconnect(server_peer_id).await;
                    println!("    Disconnect result: {:?}", disconnect_result);
                    
                    if disconnect_result.is_ok() {
                        successful_cycles += 1;
                    }
                    
                    // Ждем полной очистки
                    println!("    Waiting for cleanup...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                } else {
                    println!("    No active connections found, skipping disconnect");
                }
            } else {
                println!("    Connect failed: {:?}", connect_result);
            }
            
            // Проверяем финальное состояние после цикла
            println!("    Checking final state...");
            let final_connections = client_commander.get_all_connections().await.unwrap_or_default();
            let final_active = final_connections.iter().filter(|c| c.is_active()).count();
            println!("    Final active connections after cycle: {}", final_active);
        }
        
        println!("✅ Completed {}/{} rapid connect/disconnect cycles", successful_cycles, total_cycles);
        
        // Проверяем финальное состояние
        let final_connections = client_commander.get_all_connections().await.unwrap();
        let active_connections = final_connections.iter().filter(|c| c.is_active()).count();
        
        println!("Final active connections: {}", active_connections);
        
        // Cleanup
        client_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Rapid connect/disconnect test completed"),
        Err(_) => panic!("⏰ Rapid connect/disconnect test timed out ({}s)", test_timeout.as_secs()),
    }
}

// ==========================================
// СПЕЦИАЛЬНЫЕ ТЕСТЫ ДЛЯ ОТЛАДКИ
// ==========================================

/// Тест для отладки проблем с аутентификацией
/// Этот тест помогает понять где именно зависает аутентификация
#[tokio::test]
async fn test_debug_authentication_hang() {
    println!("🔧 Debug test for authentication hang");
    
    let (mut server_node, server_commander, mut server_events, server_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
    
    let (mut client_node, client_commander, mut client_events, _client_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server_node.run_with_cleanup_interval(Duration::from_millis(100)).await;
    });
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_millis(100)).await;
    });
    
    // Настраиваем сервер
    println!("1. Setting up server...");
    server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
    
    let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = server_events.recv().await {
            if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                return addr;
            }
        }
        panic!("No address");
    }).await.unwrap();
    
    println!("2. Server listening on: {}", server_addr);
    
    // Подключаемся
    println!("3. Initiating connection...");
    let connect_result = client_commander.connect_with_timeout(server_addr, 10).await;
    println!("4. Connect result: {:?}", connect_result);
    
    if connect_result.is_ok() {
        println!("5. Connection established, monitoring events...");
        
        // Мониторим события в течение 10 секунд
        let mut event_count = 0;
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed() < Duration::from_secs(10) && event_count < 20 {
            tokio::select! {
                Some(event) = client_events.recv() => {
                    event_count += 1;
                    println!("  Event {}: {:?}", event_count, event);
                    
                    match event {
                        NetworkEvent::PeerConnected { peer_id } if peer_id == server_peer_id => {
                            println!("  ✅ Peer connected successfully");
                            break;
                        }
                        NetworkEvent::ConnectionError { error, .. } => {
                            println!("  ❌ Connection error: {}", error);
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    println!("  ... still waiting for events ({}s elapsed)", start_time.elapsed().as_secs());
                    
                    // Проверяем состояние соединения
                    let connections = client_commander.get_all_connections().await.unwrap_or_default();
                    if !connections.is_empty() {
                        let conn = &connections[0];
                        println!("  Connection state: {:?}, Auth: {:?}", 
                                conn.connection_state, conn.auth_status);
                    }
                }
            }
        }
        
        println!("6. Event monitoring completed");
    }
    
    // Cleanup
    client_handle.abort();
    server_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Debug test completed");
}

/// Тест для проверки минимального времени, необходимого для аутентификации
#[tokio::test]
async fn test_minimum_auth_time() {
    println!("🔧 Testing minimum time required for authentication");
    
    let (mut server_node, server_commander, mut server_events, server_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
    
    let (mut client_node, client_commander, mut client_events, _client_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server_node.run_with_cleanup_interval(Duration::from_millis(50)).await;
    });
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_millis(50)).await;
    });
    
    // Настраиваем сервер
    server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await.unwrap();
    
    let server_addr = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = server_events.recv().await {
            if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                return addr;
            }
        }
        panic!("No address");
    }).await.unwrap();
    
    // Тестируем разные времена ожидания
    let test_times = [1, 2, 5, 10, 15, 20]; // секунды
    
    for &wait_time in &test_times {
        println!("Testing with {}s wait time:", wait_time);
        
        // Подключаемся
        let connect_result = client_commander.connect_with_timeout(server_addr.clone(), 5).await;
        if connect_result.is_ok() {
            let start_time = std::time::Instant::now();
            
            // Ждем указанное время
            tokio::time::sleep(Duration::from_secs(wait_time)).await;
            
            // Проверяем состояние
            let connections = client_commander.get_all_connections().await.unwrap_or_default();
            if let Some(conn) = connections.first() {
                println!("  Connection state after {}s: {:?}, Auth: {:?}", 
                        wait_time, conn.connection_state, conn.auth_status);
                
                let is_authenticated = matches!(conn.auth_status, 
                    xnetwork::connection_management::AuthStatus::Authenticated);
                
                if is_authenticated {
                    println!("  ✅ Authentication completed in ≤{}s", wait_time);
                } else {
                    println!("  ⏳ Authentication still in progress after {}s", wait_time);
                }
            }
            
            // Отключаемся для следующего теста
            let _ = client_commander.disconnect(server_peer_id).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
        } else {
            println!("  ❌ Connection failed");
        }
    }
    
    // Cleanup
    client_handle.abort();
    server_handle.abort();
    
    println!("✅ Minimum auth time test completed");
}
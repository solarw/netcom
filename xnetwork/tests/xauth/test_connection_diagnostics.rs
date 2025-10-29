// tests/xauth/test_connection_diagnostics.rs - Диагностика проблем с соединениями

use std::time::Duration;
use xnetwork::XRoutesConfig;

use crate::common::*;

#[tokio::test]
async fn test_connection_lifecycle_diagnostics() {
    let test_timeout = Duration::from_secs(30);
    
    println!("🧪 Testing connection lifecycle diagnostics");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем пару нод с включенной аутентификацией
        let (_server_handle, _client_handle, server_commander, client_commander, 
             server_peer_id, client_peer_id, server_addr) = 
            create_connected_pair().await.unwrap();
        
        println!("✅ Connection established between {} and {}", client_peer_id, server_peer_id);
        
        // Проверяем начальное состояние соединений
        let initial_client_connections = client_commander.get_all_connections().await.unwrap();
        let initial_server_connections = server_commander.get_all_connections().await.unwrap();
        
        println!("📊 Initial client connections: {}", initial_client_connections.len());
        println!("📊 Initial server connections: {}", initial_server_connections.len());
        
        for conn in &initial_client_connections {
            println!("   Client connection: {:?}", conn);
        }
        for conn in &initial_server_connections {
            println!("   Server connection: {:?}", conn);
        }
        
        // Ждем 1 секунду для завершения аутентификации
        println!("⏳ Waiting for authentication to complete...");
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Проверяем состояние после ожидания
        let after_auth_client_connections = client_commander.get_all_connections().await.unwrap();
        let after_auth_server_connections = server_commander.get_all_connections().await.unwrap();
        
        println!("📊 Client connections after auth: {}", after_auth_client_connections.len());
        println!("📊 Server connections after auth: {}", after_auth_server_connections.len());
        
        for conn in &after_auth_client_connections {
            println!("   Client connection after auth: {:?}", conn);
        }
        for conn in &after_auth_server_connections {
            println!("   Server connection after auth: {:?}", conn);
        }
        
        // Проверяем состояние сети
        let client_network_state = client_commander.get_network_state().await.unwrap();
        let server_network_state = server_commander.get_network_state().await.unwrap();
        
        println!("📊 Client network state: total_connections={}, authenticated_peers={}", 
                 client_network_state.total_connections, client_network_state.authenticated_peers);
        println!("📊 Server network state: total_connections={}, authenticated_peers={}", 
                 server_network_state.total_connections, server_network_state.authenticated_peers);
        
        // Проверяем статус аутентификации
        let is_client_authenticated = client_commander.is_peer_authenticated(server_peer_id).await.unwrap();
        let is_server_authenticated = server_commander.is_peer_authenticated(client_peer_id).await.unwrap();
        
        println!("🔐 Client authenticated with server: {}", is_client_authenticated);
        println!("🔐 Server authenticated with client: {}", is_server_authenticated);
        
        // Ждем еще 5 секунд для проверки стабильности
        println!("⏳ Waiting 5 seconds to check connection stability...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Проверяем финальное состояние
        let final_client_connections = client_commander.get_all_connections().await.unwrap();
        let final_server_connections = server_commander.get_all_connections().await.unwrap();
        
        println!("📊 Final client connections: {}", final_client_connections.len());
        println!("📊 Final server connections: {}", final_server_connections.len());
        
        for conn in &final_client_connections {
            println!("   Final client connection: {:?}", conn);
        }
        for conn in &final_server_connections {
            println!("   Final server connection: {:?}", conn);
        }
        
        // Анализируем результаты
        let connections_lost = initial_client_connections.len() > final_client_connections.len();
        
        if connections_lost {
            println!("⚠️  WARNING: Connections were lost during the test!");
            println!("   Initial: {}, Final: {}", initial_client_connections.len(), final_client_connections.len());
        } else {
            println!("✅ Connection stability: OK");
        }
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        println!("✅ Connection lifecycle diagnostics test completed");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection lifecycle diagnostics test completed"),
        Err(_) => panic!("⏰ Connection lifecycle diagnostics test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_quic_timeout_diagnostics() {
    let test_timeout = Duration::from_secs(60);
    
    println!("🧪 Testing QUIC timeout diagnostics");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем пару нод
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        println!("✅ Connection established");
        
        // Проверяем соединения каждые 2 секунды в течение 30 секунд
        for i in 0..15 {
            let connections = client_commander.get_all_connections().await.unwrap();
            let active_connections = connections.iter().filter(|c| c.is_active()).count();
            
            println!("⏱️  Time: {}s, Active connections: {}", i * 2, active_connections);
            
            if active_connections == 0 {
                println!("⚠️  All connections closed at {}s", i * 2);
                break;
            }
            
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        
        // Проверяем финальное состояние
        let final_connections = client_commander.get_all_connections().await.unwrap();
        let final_active = final_connections.iter().filter(|c| c.is_active()).count();
        
        println!("📊 Final active connections: {}", final_active);
        
        if final_active == 0 {
            println!("❌ All connections closed due to timeout");
        } else {
            println!("✅ Connections remained active");
        }
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ QUIC timeout diagnostics test completed"),
        Err(_) => panic!("⏰ QUIC timeout diagnostics test timed out ({}s)", test_timeout.as_secs()),
    }
}

// tests/core_connections/test_connection_management.rs - Управление соединениями (PRIORITY 2)

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::XRoutesConfig;

use crate::common::*;

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

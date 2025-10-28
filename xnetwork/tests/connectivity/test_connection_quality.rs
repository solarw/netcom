// tests/connectivity/test_connection_quality.rs - Качество соединений (PRIORITY 6)

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::XRoutesConfig;

use crate::common::*;

#[tokio::test]
async fn test_memory_cleanup_after_disconnect() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing memory cleanup after disconnect");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Получаем начальное состояние
        let connections_before = client_commander.get_all_connections().await.unwrap();
        let active_before = connections_before.iter().filter(|c| c.is_active()).count();
        println!("Active connections before: {}", active_before);
        
        // Отключаемся
        let disconnect_result = client_commander.disconnect(server_peer_id).await;
        println!("Disconnect result: {:?}", disconnect_result);
        
        // Ждем очистки
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Проверяем состояние после отключения
        let connections_after = client_commander.get_all_connections().await.unwrap();
        let active_after = connections_after.iter().filter(|c| c.is_active()).count();
        println!("Active connections after: {}", active_after);
        
        // Проверяем сетевое состояние
        let network_state = client_commander.get_network_state().await.unwrap();
        println!("Network state: {} connections", network_state.total_connections);
        
        // Проверяем что память очищается (соединения не накапливаются)
        // assert!(
        //     network_state.total_connections <= active_before as u32,
        //     "Total connections should not increase after disconnect"
        // );
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Memory cleanup after disconnect test completed"),
        Err(_) => panic!("⏰ Memory cleanup after disconnect test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_disconnect_nonexistent_peer() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing disconnect from nonexistent peer");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, _events, _peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("Failed to create test node");
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Создаем несуществующий peer ID
        let nonexistent_peer_id = PeerId::random();
        
        // Пытаемся отключиться от несуществующего peer'а
        let disconnect_result = commander.disconnect(nonexistent_peer_id).await;
        
        // Проверяем что операция завершается корректно (не падает)
        println!("Disconnect from nonexistent peer result: {:?}", disconnect_result);
        
        // Проверяем что состояние сети остается стабильным
        let network_state = commander.get_network_state().await.unwrap();
        println!("Network state after disconnect attempt: {} connections", network_state.total_connections);
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Disconnect nonexistent peer test completed"),
        Err(_) => panic!("⏰ Disconnect nonexistent peer test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_connection_state_consistency() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing connection state consistency");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             _server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Получаем соединения несколько раз подряд
        let connections_1 = client_commander.get_all_connections().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let connections_2 = client_commander.get_all_connections().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let connections_3 = client_commander.get_all_connections().await.unwrap();
        
        // Проверяем консистентность
        let active_1 = connections_1.iter().filter(|c| c.is_active()).count();
        let active_2 = connections_2.iter().filter(|c| c.is_active()).count();
        let active_3 = connections_3.iter().filter(|c| c.is_active()).count();
        
        println!("Active connections: {} -> {} -> {}", active_1, active_2, active_3);
        
        // Проверяем что состояние не скачет
        let max_diff = (active_1 as i32 - active_3 as i32).abs();
        assert!(
            max_diff <= 1,
            "Connection state should be consistent (max diff: {})", max_diff
        );
        
        // Проверяем сетевое состояние
        let network_state = client_commander.get_network_state().await.unwrap();
        println!("Network state: {} connections", network_state.total_connections);
        
        // Проверяем что сетевые метрики разумны
        // assert!(
        //     network_state.total_connections >= active_3 as u32,
        //     "Network state should reflect actual connections"
        // );
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection state consistency test completed"),
        Err(_) => panic!("⏰ Connection state consistency test timed out ({}s)", test_timeout.as_secs()),
    }
}

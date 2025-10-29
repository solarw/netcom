// tests/xauth/test_connection_tracking.rs - Отслеживание проблем с трекингом соединений

use std::time::Duration;
use xnetwork::XRoutesConfig;

use crate::common::*;

#[tokio::test]
async fn test_connection_tracking_diagnostics() {
    let test_timeout = Duration::from_secs(30);
    
    println!("🧪 Testing connection tracking diagnostics");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем пару нод с включенной аутентификацией
        let (_server_handle, _client_handle, server_commander, client_commander, 
             server_peer_id, client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        println!("✅ Connection established between {} and {}", client_peer_id, server_peer_id);
        
        // Даем время для обработки событий
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Проверяем состояние соединений через разные методы
        
        // 1. Старый метод (legacy)
        let legacy_client_peers = client_commander.get_connected_peers_simple().await.unwrap();
        let legacy_server_peers = server_commander.get_connected_peers_simple().await.unwrap();
        
        println!("📊 Legacy client peers: {}", legacy_client_peers.len());
        println!("📊 Legacy server peers: {}", legacy_server_peers.len());
        
        for (peer_id, addrs) in &legacy_client_peers {
            println!("   Client legacy peer: {} -> {:?}", peer_id, addrs);
        }
        for (peer_id, addrs) in &legacy_server_peers {
            println!("   Server legacy peer: {} -> {:?}", peer_id, addrs);
        }
        
        // 2. Новый метод (connection management)
        let new_client_connections = client_commander.get_all_connections().await.unwrap();
        let new_server_connections = server_commander.get_all_connections().await.unwrap();
        
        println!("📊 New client connections: {}", new_client_connections.len());
        println!("📊 New server connections: {}", new_server_connections.len());
        
        for conn in &new_client_connections {
            println!("   Client connection: {:?}", conn);
        }
        for conn in &new_server_connections {
            println!("   Server connection: {:?}", conn);
        }
        
        // 3. Peer info
        let client_peer_info = client_commander.get_peer_info(server_peer_id).await.unwrap();
        let server_peer_info = server_commander.get_peer_info(client_peer_id).await.unwrap();
        
        println!("📊 Client peer info: {:?}", client_peer_info);
        println!("📊 Server peer info: {:?}", server_peer_info);
        
        // 4. Connected peers (new)
        let client_connected_peers = client_commander.get_connected_peers().await.unwrap();
        let server_connected_peers = server_commander.get_connected_peers().await.unwrap();
        
        println!("📊 Client connected peers (new): {}", client_connected_peers.len());
        println!("📊 Server connected peers (new): {}", server_connected_peers.len());
        
        for peer in &client_connected_peers {
            println!("   Client connected peer: {:?}", peer);
        }
        for peer in &server_connected_peers {
            println!("   Server connected peer: {:?}", peer);
        }
        
        // 5. Network state
        let client_network_state = client_commander.get_network_state().await.unwrap();
        let server_network_state = server_commander.get_network_state().await.unwrap();
        
        println!("📊 Client network state: {:?}", client_network_state);
        println!("📊 Server network state: {:?}", server_network_state);
        
        // Анализируем проблему
        let legacy_has_connections = !legacy_client_peers.is_empty();
        let new_has_connections = !new_client_connections.is_empty();
        
        println!("🔍 Analysis:");
        println!("   Legacy tracking has connections: {}", legacy_has_connections);
        println!("   New tracking has connections: {}", new_has_connections);
        
        if legacy_has_connections && !new_has_connections {
            println!("❌ PROBLEM: Legacy tracking works but new connection management doesn't!");
            println!("   This means ConnectionEstablished events are processed but connections aren't added to self.connections");
        } else if !legacy_has_connections && !new_has_connections {
            println!("❌ PROBLEM: Neither tracking method works!");
            println!("   This means ConnectionEstablished events aren't being processed at all");
        } else if legacy_has_connections && new_has_connections {
            println!("✅ Both tracking methods work correctly");
        }
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        println!("✅ Connection tracking diagnostics test completed");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection tracking diagnostics test completed"),
        Err(_) => panic!("⏰ Connection tracking diagnostics test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_connection_events_processing() {
    let test_timeout = Duration::from_secs(30);
    
    println!("🧪 Testing connection events processing");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем пару нод
        let (_server_handle, _client_handle, server_commander, client_commander, 
             server_peer_id, client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        println!("✅ Connection established");
        
        // Проверяем состояние сразу после соединения
        let immediate_client_connections = client_commander.get_all_connections().await.unwrap();
        let immediate_server_connections = server_commander.get_all_connections().await.unwrap();
        
        println!("📊 Immediate client connections: {}", immediate_client_connections.len());
        println!("📊 Immediate server connections: {}", immediate_server_connections.len());
        
        // Ждем немного для обработки событий
        println!("⏳ Waiting for event processing...");
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Проверяем состояние после ожидания
        let delayed_client_connections = client_commander.get_all_connections().await.unwrap();
        let delayed_server_connections = server_commander.get_all_connections().await.unwrap();
        
        println!("📊 Delayed client connections: {}", delayed_client_connections.len());
        println!("📊 Delayed server connections: {}", delayed_server_connections.len());
        
        // Проверяем, изменилось ли состояние
        let client_changed = immediate_client_connections.len() != delayed_client_connections.len();
        let server_changed = immediate_server_connections.len() != delayed_server_connections.len();
        
        println!("🔍 Event processing analysis:");
        println!("   Client connections changed: {}", client_changed);
        println!("   Server connections changed: {}", server_changed);
        
        if client_changed || server_changed {
            println!("✅ Connection events are being processed asynchronously");
        } else {
            println!("⚠️  Connection events may not be processed correctly");
        }
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        println!("✅ Connection events processing test completed");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Connection events processing test completed"),
        Err(_) => panic!("⏰ Connection events processing test timed out ({}s)", test_timeout.as_secs()),
    }
}

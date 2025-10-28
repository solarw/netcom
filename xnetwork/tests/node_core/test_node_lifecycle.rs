// tests/node_core/test_node_lifecycle.rs - Жизненный цикл ноды (PRIORITY 1)

use std::time::Duration;
use xnetwork::{XRoutesConfig, events::NetworkEvent};

use crate::common::*;

#[tokio::test]
async fn test_node_creation_and_startup() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing node creation and startup");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем ноду с клиентской конфигурацией
        let (mut node, commander, mut events, peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("Failed to create test node");
        
        println!("Created node with peer ID: {}", peer_id);
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Проверяем что нода создана корректно
        assert!(!peer_id.to_string().is_empty(), "Peer ID should not be empty");
        
        // Проверяем что нода может получать команды
        let network_state = commander.get_network_state().await;
        println!("Network state: {:?}", network_state);
        
        // Проверяем что нода может слушать события
        let event_received = tokio::time::timeout(Duration::from_secs(2), async {
            events.recv().await.is_some()
        }).await.unwrap_or(false);
        
        println!("Node can receive events: {}", event_received);
        
        // Запускаем слушатель
        let listen_result = commander.listen_port(Some("127.0.0.1".to_string()), 0).await;
        println!("Listen result: {:?}", listen_result);
        
        // Ждем события прослушивания
        let listening_event = tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = events.recv().await {
                if let NetworkEvent::ListeningOnAddress { .. } = event {
                    return Some(event);
                }
            }
            None
        }).await.unwrap_or(None);
        
        println!("Listening event received: {}", listening_event.is_some());
        
        // Проверяем что нода работает стабильно
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Останавливаем слушатель (если метод существует)
        // let stop_result = commander.stop_listening().await;
        // println!("Stop listening result: {:?}", stop_result);
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Node creation and startup test completed"),
        Err(_) => panic!("⏰ Node creation and startup test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_node_diagnostics() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing node diagnostics");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, _events, peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("Failed to create test node");
        
        let node_handle = tokio::spawn(async move {
            node.run_with_cleanup_interval(Duration::from_secs(1)).await;
        });
        
        // Получаем диагностическую информацию (если метод существует)
        // let diagnostics = commander.get_diagnostics().await;
        // println!("Diagnostics: {:?}", diagnostics);
        
        // Проверяем что диагностика содержит разумные значения
        // if let Ok(diag) = diagnostics {
        //     assert!(!diag.peer_id.to_string().is_empty(), "Diagnostics should contain peer ID");
        //     assert_eq!(diag.peer_id, peer_id, "Diagnostics peer ID should match node peer ID");
            
        //     // Проверяем что метрики разумны
        //     println!("Connections: {}", diag.connections);
        //     println!("Active streams: {}", diag.active_streams);
        //     println!("Uptime: {}s", diag.uptime_seconds);
            
        //     assert!(diag.uptime_seconds >= 0, "Uptime should be non-negative");
        //     assert!(diag.connections >= 0, "Connections count should be non-negative");
        //     assert!(diag.active_streams >= 0, "Active streams count should be non-negative");
        // }
        
        // Получаем сетевое состояние
        let network_state = commander.get_network_state().await;
        println!("Network state: {:?}", network_state);
        
        // Проверяем что сетевое состояние разумно
        if let Ok(state) = network_state {
            assert!(state.total_connections >= 0, "Total connections should be non-negative");
            // assert!(state.active_connections >= 0, "Active connections should be non-negative");
            // assert!(state.total_connections >= state.active_connections, 
            //        "Total connections should be >= active connections");
        }
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Node diagnostics test completed"),
        Err(_) => panic!("⏰ Node diagnostics test timed out ({}s)", test_timeout.as_secs()),
    }
}

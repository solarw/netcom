// tests/xauth/test_node_execution.rs - Проверка выполнения ноды

use std::time::Duration;
use xnetwork::XRoutesConfig;

use crate::test_utils::{TestNode, NodeExt};

#[tokio::test]
async fn test_node_execution_loop() {
    let test_timeout = Duration::from_secs(30);
    
    println!("🧪 Testing node execution loop");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем пару нод, но не запускаем их через spawn
        let mut server_node = TestNode::new_ephemeral().await.unwrap();
        let mut client_node = TestNode::new_ephemeral().await.unwrap();
        
        let server_commander = server_node.commander;
        let client_commander = client_node.commander;
        let mut server_events = server_node.events;
        let mut client_events = client_node.events;
        let server_peer_id = server_node.peer_id;
        let client_peer_id = client_node.peer_id;
        
        println!("✅ Nodes created (not spawned yet)");
        
        // Запускаем ноды в отдельных задачах
        let server_handle = tokio::spawn(async move {
            println!("🔄 Starting server node execution loop");
            server_node.node.run().await;
            println!("🛑 Server node execution loop stopped");
        });
        
        let client_handle = tokio::spawn(async move {
            println!("🔄 Starting client node execution loop");
            client_node.node.run().await;
            println!("🛑 Client node execution loop stopped");
        });
        
        // Даем время для запуска циклов выполнения
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Запускаем сервер на прослушивание
        println!("🎯 Starting server listening...");
        server_commander.listen_port(None, 0).await.unwrap();
        
        // Ждем получения адреса прослушивания
        println!("⏳ Waiting for server listening address...");
        let mut server_addr = None;
        for _ in 0..20 {
            let server_addrs = server_commander.get_listen_addresses().await.unwrap();
            if let Some(addr) = server_addrs.first() {
                server_addr = Some(addr.clone());
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        let server_addr = server_addr.expect("Failed to get server listening address");
        println!("🎯 Server listening on: {}", server_addr);
        
        // Подключаем клиент к серверу
        println!("🎯 Connecting client to server...");
        client_commander.connect(server_addr).await.unwrap();
        
        // Ждем событий соединения
        println!("⏳ Waiting for connection events...");
        
        let mut server_connection_events = 0;
        let mut client_connection_events = 0;
        
        // Ожидаем события в течение 5 секунд
        for _ in 0..50 {
            // Проверяем события сервера
            match server_events.try_recv() {
                Ok(event) => {
                    println!("📡 Server event: {:?}", event);
                    if matches!(event, xnetwork::NetworkEvent::ConnectionOpened { .. }) {
                        server_connection_events += 1;
                    }
                }
                Err(_) => {
                    // Нет событий в канале - это нормально
                }
            }
            
            // Проверяем события клиента
            match client_events.try_recv() {
                Ok(event) => {
                    println!("📡 Client event: {:?}", event);
                    if matches!(event, xnetwork::NetworkEvent::ConnectionOpened { .. }) {
                        client_connection_events += 1;
                    }
                }
                Err(_) => {
                    // Нет событий в канале - это нормально
                }
            }
            
            if server_connection_events > 0 && client_connection_events > 0 {
                break;
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        println!("📊 Server connection events: {}", server_connection_events);
        println!("📊 Client connection events: {}", client_connection_events);
        
        // Проверяем состояние соединений
        let server_connections = server_commander.get_all_connections().await.unwrap();
        let client_connections = client_commander.get_all_connections().await.unwrap();
        
        println!("📊 Server connections: {}", server_connections.len());
        println!("📊 Client connections: {}", client_connections.len());
        
        // Анализируем результаты
        let events_received = server_connection_events > 0 && client_connection_events > 0;
        let connections_tracked = !server_connections.is_empty() && !client_connections.is_empty();
        
        println!("🔍 Execution analysis:");
        println!("   Connection events received: {}", events_received);
        println!("   Connections tracked: {}", connections_tracked);
        
        if events_received && !connections_tracked {
            println!("❌ PROBLEM: Events received but connections not tracked!");
            println!("   This means handle_swarm_event is called but add_connection is not working");
        } else if !events_received && !connections_tracked {
            println!("❌ PROBLEM: No events received!");
            println!("   This means the execution loop is not processing Swarm events");
        } else if events_received && connections_tracked {
            println!("✅ Everything works correctly!");
        }
        
        // Останавливаем ноды
        println!("🛑 Shutting down nodes...");
        server_commander.shutdown().await.unwrap();
        client_commander.shutdown().await.unwrap();
        
        // Ждем завершения задач
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Отменяем задачи (они должны завершиться после shutdown)
        server_handle.abort();
        client_handle.abort();
        
        println!("✅ Node execution loop test completed");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Node execution loop test completed"),
        Err(_) => panic!("⏰ Node execution loop test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_swarm_event_processing() {
    let test_timeout = Duration::from_secs(30);
    
    println!("🧪 Testing Swarm event processing");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем простую ноду для тестирования обработки событий
        let mut node = TestNode::new_ephemeral().await.unwrap();
        let commander = node.commander;
        let mut events = node.events;
        let _peer_id = node.peer_id;
        
        // Запускаем ноду в отдельной задаче
        let node_handle = tokio::spawn(async move {
            println!("🔄 Starting node execution loop");
            node.node.run().await;
            println!("🛑 Node execution loop stopped");
        });
        
        // Даем время для запуска
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Запускаем прослушивание
        commander.listen_port(None, 0).await.unwrap();
        
        // Получаем адрес
        let addrs = commander.get_listen_addresses().await.unwrap();
        println!("🎯 Node listening on: {:?}", addrs);
        
        // Проверяем, что мы получаем события прослушивания
        let mut listening_event_received = false;
        
        for _ in 0..20 {
            match events.try_recv() {
                Ok(event) => {
                    println!("📡 Event received: {:?}", event);
                    if matches!(event, xnetwork::NetworkEvent::ListeningOnAddress { .. }) {
                        listening_event_received = true;
                        break;
                    }
                }
                Err(_) => {
                    // Нет событий в канале - это нормально
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        println!("📊 Listening event received: {}", listening_event_received);
        
        if listening_event_received {
            println!("✅ Swarm events are being processed correctly");
        } else {
            println!("❌ PROBLEM: No Swarm events received!");
            println!("   This means the execution loop is not working properly");
        }
        
        // Останавливаем ноду
        commander.shutdown().await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        node_handle.abort();
        
        println!("✅ Swarm event processing test completed");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Swarm event processing test completed"),
        Err(_) => panic!("⏰ Swarm event processing test timed out ({}s)", test_timeout.as_secs()),
    }
}

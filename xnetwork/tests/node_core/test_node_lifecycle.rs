//! Тест жизненного цикла ноды NetCom
//! 
//! Этот тест проверяет полный жизненный цикл ноды от создания до завершения:
//! - Создание ноды
//! - Запуск прослушивания порта
//! - Получение сетевого состояния
//! - Корректное завершение работы
//! 
//! PRIORITY 1: CORE - критический функционал

use std::time::Duration;

use crate::utils::node::create_node;
use crate::utils::event_handlers::create_listening_address_handler;

#[tokio::test]
async fn test_node_lifecycle_basic() {
    println!("🧪 Testing basic node lifecycle");
    
    let test_timeout = Duration::from_secs(10);
    
    let result = tokio::time::timeout(test_timeout, async {
        // ✅ ПРАВИЛЬНО: создаем ноду
        println!("🔄 Создаем ноду...");
        let (commander, mut events, handle, peer_id) = 
            create_node().await.expect("Failed to create node");
        
        println!("✅ Нода создана: {:?}", peer_id);
        
        // ✅ ПРАВИЛЬНО: создаем обработчики событий
        println!("🔄 Создаем обработчики событий...");
        let (listening_rx, mut listening_handler) = 
            create_listening_address_handler();
        
        // ✅ ПРАВИЛЬНО: сначала запускаем обработку событий
        println!("🔄 Запускаем обработку событий...");
        let events_task = tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                println!("📡 NODE EVENT: {:?}", event);
                // обрабатывает событие прослушивания
                listening_handler(&event);
            }
        });
        
        // ✅ ПРАВИЛЬНО: потом операции с командой
        
        // Запускаем прослушивание порта
        println!("🔄 Запускаем прослушивание порта...");
        commander.listen_port(Some("127.0.0.1".to_string()), 0).await
            .expect("Failed to start listening");
        
        // ✅ ПРАВИЛЬНО: ожидаем события прослушивания
        println!("⏳ Ожидаем события прослушивания...");
        let listening_addr = tokio::time::timeout(Duration::from_secs(5), listening_rx).await
            .expect("Node should start listening within timeout")
            .expect("Failed to get listening address");
        
        println!("✅ Нода запущена на адресе: {}", listening_addr);
        
        // Проверяем сетевой статус
        println!("🔄 Получаем сетевой статус...");
        let network_state = commander.get_network_state().await
            .expect("Failed to get network state");
        
        println!("📊 СЕТЕВОЙ СТАТУС:");
        println!("   Local Peer ID: {:?}", network_state.local_peer_id);
        println!("   Listening addresses: {:?}", network_state.listening_addresses);
        println!("   Total connections: {:?}", network_state.total_connections);
        println!("   Authenticated peers: {:?}", network_state.authenticated_peers);
        
        // Проверяем, что нода действительно слушает
        assert!(!network_state.listening_addresses.is_empty(), 
                "Node should have listening addresses");
        assert!(network_state.listening_addresses.contains(&listening_addr),
                "Listening address should be in network state");
        
        // Проверяем, что нода имеет корректный Peer ID
        assert_eq!(network_state.local_peer_id, peer_id, 
                   "Network state should have correct peer ID");
        
        // ✅ ПРАВИЛЬНО: корректное завершение работы
        println!("🔄 Завершаем работу ноды...");
        commander.shutdown().await.expect("Failed to shutdown node");
        
        // Ждем завершения задачи обработки событий
        let _ = tokio::join!(handle, events_task);
        
        println!("✅ Node lifecycle test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ Node lifecycle test completed successfully"),
        Ok(Err(e)) => panic!("❌ Node lifecycle test failed: {}", e),
        Err(_) => panic!("⏰ Node lifecycle test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_node_lifecycle_multiple_restarts() {
    println!("🧪 Testing node lifecycle with multiple restarts");
    
    let test_timeout = Duration::from_secs(15);
    
    let result = tokio::time::timeout(test_timeout, async {
        // Тестируем несколько циклов создания/завершения
        for i in 0..3 {
            println!("🔄 Цикл {}: создаем ноду...", i + 1);
            let (commander, mut events, handle, peer_id) = 
                create_node().await.expect("Failed to create node");
            
            println!("✅ Цикл {}: нода создана: {:?}", i + 1, peer_id);
            
            // Создаем обработчики
            let (listening_rx, mut listening_handler) = 
                create_listening_address_handler();
            
            // Запускаем обработку событий
            let events_task = tokio::spawn(async move {
                while let Some(event) = events.recv().await {
                    println!("📡 CYCLE {} EVENT: {:?}", i + 1, event);
                    listening_handler(&event);
                }
            });
            
            // Запускаем прослушивание
            commander.listen_port(Some("127.0.0.1".to_string()), 0).await
                .expect("Failed to start listening");
            
            // Ожидаем события прослушивания
            let listening_addr = tokio::time::timeout(Duration::from_secs(5), listening_rx).await
                .expect("Node should start listening within timeout")
                .expect("Failed to get listening address");
            
            println!("✅ Цикл {}: нода запущена на {}", i + 1, listening_addr);
            
            // Проверяем состояние
            let network_state = commander.get_network_state().await
                .expect("Failed to get network state");
            
            assert!(!network_state.listening_addresses.is_empty(), 
                    "Node should have listening addresses in cycle {}", i + 1);
            
            // Корректно завершаем
            commander.shutdown().await.expect("Failed to shutdown node");
            
            // Ждем завершения
            let _ = tokio::join!(handle, events_task);
            
            println!("✅ Цикл {} завершен успешно", i + 1);
            
            // Небольшая пауза между циклами
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        println!("✅ Multiple restarts test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ Multiple restarts test completed successfully"),
        Ok(Err(e)) => panic!("❌ Multiple restarts test failed: {}", e),
        Err(_) => panic!("⏰ Multiple restarts test timed out ({}s)", test_timeout.as_secs()),
    }
}

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
async fn test_node_restart() {
    println!("🧪 Testing node restart functionality");
    
    let test_timeout = Duration::from_secs(15);
    
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
        
        assert!(!network_state.listening_addresses.is_empty(), 
                "Node should have listening addresses");
        
        // ✅ ПРАВИЛЬНО: корректное завершение работы
        println!("🔄 Завершаем работу ноды...");
        commander.shutdown().await.expect("Failed to shutdown node");
        
        // Ждем завершения задачи обработки событий
        let _ = tokio::join!(handle, events_task);
        
        println!("✅ Первый цикл завершен, готовимся к перезапуску...");
        
        // Небольшая пауза перед перезапуском
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // ✅ ПРАВИЛЬНО: перезапускаем ноду
        println!("🔄 Перезапускаем ноду...");
        let (commander2, mut events2, handle2, peer_id2) = 
            create_node().await.expect("Failed to create node after restart");
        
        println!("✅ Нода перезапущена: {:?}", peer_id2);
        
        // ✅ ПРАВИЛЬНО: создаем обработчики событий для перезапущенной ноды
        println!("🔄 Создаем обработчики событий для перезапущенной ноды...");
        let (listening_rx2, mut listening_handler2) = 
            create_listening_address_handler();
        
        // ✅ ПРАВИЛЬНО: сначала запускаем обработку событий
        println!("🔄 Запускаем обработку событий перезапущенной ноды...");
        let events_task2 = tokio::spawn(async move {
            while let Some(event) = events2.recv().await {
                println!("📡 RESTARTED NODE EVENT: {:?}", event);
                listening_handler2(&event);
            }
        });
        
        // ✅ ПРАВИЛЬНО: потом операции с командой
        
        // Запускаем прослушивание порта для перезапущенной ноды
        println!("🔄 Запускаем прослушивание порта для перезапущенной ноды...");
        commander2.listen_port(Some("127.0.0.1".to_string()), 0).await
            .expect("Failed to start listening after restart");
        
        // ✅ ПРАВИЛЬНО: ожидаем события прослушивания
        println!("⏳ Ожидаем события прослушивания перезапущенной ноды...");
        let listening_addr2 = tokio::time::timeout(Duration::from_secs(5), listening_rx2).await
            .expect("Restarted node should start listening within timeout")
            .expect("Failed to get listening address after restart");
        
        println!("✅ Перезапущенная нода запущена на адресе: {}", listening_addr2);
        
        // Проверяем сетевой статус перезапущенной ноды
        println!("🔄 Получаем сетевой статус перезапущенной ноды...");
        let network_state2 = commander2.get_network_state().await
            .expect("Failed to get network state after restart");
        
        println!("📊 СЕТЕВОЙ СТАТУС ПЕРЕЗАПУЩЕННОЙ НОДЫ:");
        println!("   Local Peer ID: {:?}", network_state2.local_peer_id);
        println!("   Listening addresses: {:?}", network_state2.listening_addresses);
        println!("   Total connections: {:?}", network_state2.total_connections);
        println!("   Authenticated peers: {:?}", network_state2.authenticated_peers);
        
        // Проверяем, что перезапущенная нода работает корректно
        assert!(!network_state2.listening_addresses.is_empty(), 
                "Restarted node should have listening addresses");
        assert!(network_state2.listening_addresses.contains(&listening_addr2),
                "Listening address should be in network state after restart");
        
        // Проверяем, что Peer ID изменился (новая нода)
        assert_ne!(peer_id, peer_id2, 
                   "Restarted node should have different peer ID");
        
        // ✅ ПРАВИЛЬНО: корректное завершение работы перезапущенной ноды
        println!("🔄 Завершаем работу перезапущенной ноды...");
        commander2.shutdown().await.expect("Failed to shutdown restarted node");
        
        // Ждем завершения задачи обработки событий
        let _ = tokio::join!(handle2, events_task2);
        
        println!("✅ Node restart test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ Node restart test completed successfully"),
        Ok(Err(e)) => panic!("❌ Node restart test failed: {}", e),
        Err(_) => panic!("⏰ Node restart test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_network_state_after_listen() {
    println!("🧪 Testing network state after listening");
    
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
                listening_handler(&event);
            }
        });
        
        // ✅ ПРАВИЛЬНО: потом операции с командой
        
        // Проверяем начальное состояние сети (до прослушивания)
        println!("🔄 Получаем начальное сетевое состояние...");
        let initial_state = commander.get_network_state().await
            .expect("Failed to get initial network state");
        
        println!("📊 НАЧАЛЬНОЕ СЕТЕВОЕ СОСТОЯНИЕ:");
        println!("   Local Peer ID: {:?}", initial_state.local_peer_id);
        println!("   Listening addresses: {:?}", initial_state.listening_addresses);
        println!("   Total connections: {:?}", initial_state.total_connections);
        println!("   Authenticated peers: {:?}", initial_state.authenticated_peers);
        
        // Проверяем, что начальное состояние корректно
        assert_eq!(initial_state.local_peer_id, peer_id, 
                   "Initial state should have correct peer ID");
        assert!(initial_state.listening_addresses.is_empty(), 
                "Initial state should have no listening addresses");
        assert_eq!(initial_state.total_connections, 0, 
                   "Initial state should have 0 connections");
        assert_eq!(initial_state.authenticated_peers, 0, 
                "Initial state should have 0 authenticated peers");
        
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
        
        // Проверяем сетевое состояние после прослушивания
        println!("🔄 Получаем сетевое состояние после прослушивания...");
        let state_after_listen = commander.get_network_state().await
            .expect("Failed to get network state after listening");
        
        println!("📊 СЕТЕВОЕ СОСТОЯНИЕ ПОСЛЕ ПРОСЛУШИВАНИЯ:");
        println!("   Local Peer ID: {:?}", state_after_listen.local_peer_id);
        println!("   Listening addresses: {:?}", state_after_listen.listening_addresses);
        println!("   Total connections: {:?}", state_after_listen.total_connections);
        println!("   Authenticated peers: {:?}", state_after_listen.authenticated_peers);
        
        // Проверяем, что состояние изменилось корректно
        assert_eq!(state_after_listen.local_peer_id, peer_id, 
                   "State after listen should have correct peer ID");
        assert!(!state_after_listen.listening_addresses.is_empty(), 
                "State after listen should have listening addresses");
        assert!(state_after_listen.listening_addresses.contains(&listening_addr),
                "Listening address should be in network state after listen");
        assert_eq!(state_after_listen.total_connections, 0, 
                   "State after listen should still have 0 connections");
        assert_eq!(state_after_listen.authenticated_peers, 0, 
                "State after listen should still have 0 authenticated peers");
        
        // Проверяем, что количество адресов прослушивания увеличилось
        assert!(state_after_listen.listening_addresses.len() > initial_state.listening_addresses.len(),
                "Number of listening addresses should increase after listen");
        
        // ✅ ПРАВИЛЬНО: корректное завершение работы
        println!("🔄 Завершаем работу ноды...");
        commander.shutdown().await.expect("Failed to shutdown node");
        
        // Ждем завершения задачи обработки событий
        let _ = tokio::join!(handle, events_task);
        
        println!("✅ Network state after listen test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ Network state after listen test completed successfully"),
        Ok(Err(e)) => panic!("❌ Network state after listen test failed: {}", e),
        Err(_) => panic!("⏰ Network state after listen test timed out ({}s)", test_timeout.as_secs()),
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

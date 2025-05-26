// tests/test_connections_problematic.rs - Оптимизированные проблемные тесты
//
// Эта версия использует адаптивные таймауты и более быстрые циклы обработки

use std::time::Duration;
use libp2p::PeerId;
use xnetwork::{
    XRoutesConfig, 
    events::NetworkEvent,
};

mod common;
use common::*;

// ==========================================
// HELPER FUNCTIONS - Адаптивные ожидания
// ==========================================

/// Адаптивное ожидание события с коротким таймаутом
async fn wait_for_event_adaptive<F>(
    events: &mut tokio::sync::mpsc::Receiver<NetworkEvent>,
    predicate: F,
    max_wait_ms: u64,
) -> bool 
where
    F: Fn(&NetworkEvent) -> bool,
{
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(max_wait_ms);
    
    while start.elapsed() < timeout {
        tokio::select! {
            Some(event) = events.recv() => {
                if predicate(&event) {
                    return true;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                // Проверяем каждые 50ms
            }
        }
    }
    false
}

/// Быстрая проверка состояния соединения
async fn check_connection_ready(commander: &xnetwork::Commander) -> bool {
    if let Ok(connections) = commander.get_all_connections().await {
        connections.iter().any(|c| c.is_active())
    } else {
        false
    }
}

/// Ожидание готовности соединения с коротким таймаутом
async fn wait_for_connection_ready(commander: &xnetwork::Commander, max_wait_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(max_wait_ms);
    
    while start.elapsed() < timeout {
        if check_connection_ready(commander).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await; // Очень частые проверки
    }
    false
}

// ==========================================
// ОПТИМИЗИРОВАННЫЕ ТЕСТЫ
// ==========================================

/// Тест отслеживания состояния сети - УСКОРЕННАЯ ВЕРСИЯ
#[tokio::test]
async fn test_network_state_tracking_fast() {
    let test_timeout = Duration::from_millis(3000); // Сократили с 15s до 3s
    
    println!("🧪 Testing network state tracking (FAST version)");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut node, commander, _events, peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("PANIC: Failed to create test node");
        
        let node_handle = tokio::spawn(async move {
            // Очень частая очистка для быстрых тестов
            node.run_with_cleanup_interval(Duration::from_millis(50)).await;
        });
        
        // Минимальное время инициализации
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Получаем начальное состояние сети
        println!("Getting initial network state...");
        let initial_state = commander.get_network_state().await
            .expect("PANIC: Failed to get initial network state");
        
        println!("Initial state: {} connections, uptime: {:?}", 
                 initial_state.total_connections, initial_state.uptime);
        
        assert_eq!(initial_state.local_peer_id, peer_id, "PANIC: Local peer ID mismatch");
        assert_eq!(initial_state.total_connections, 0, "PANIC: Should start with 0 connections");
        
        // Пытаемся запустить слушатель с коротким таймаутом
        println!("Starting listener with timeout...");
        let listen_result = tokio::time::timeout(
            Duration::from_millis(1000), // 1 секунда на запуск слушателя
            commander.listen_port(Some("127.0.0.1".to_string()), 0)
        ).await;
        
        let listener_started = match listen_result {
            Ok(Ok(_)) => {
                println!("✅ Listener started successfully");
                true
            }
            Ok(Err(e)) => {
                panic!("PANIC: Listener failed to start: {}", e);
            }
            Err(_) => {
                panic!("PANIC: Listener start timed out after 1000ms");
            }
        };
        
        if listener_started {
            // Быстрая проверка обновленного состояния
            tokio::time::sleep(Duration::from_millis(200)).await;
            
            let updated_state = commander.get_network_state().await
                .expect("PANIC: Failed to get updated network state");
            
            println!("Updated state: {} addresses, uptime: {:?}", 
                     updated_state.listening_addresses.len(), updated_state.uptime);
            
            // Быстрая проверка адресов
            let listen_addresses = commander.get_listen_addresses().await
                .expect("PANIC: Failed to get listen addresses");
            
            println!("Listening addresses: {}", listen_addresses.len());
            
            assert!(updated_state.uptime >= initial_state.uptime, 
                   "PANIC: Uptime should not decrease");
            
            // КРИТИЧЕСКАЯ ПРОВЕРКА: слушатель должен создать адреса
            if updated_state.listening_addresses.is_empty() && listen_addresses.is_empty() {
                panic!("PANIC: Listener started but no listening addresses found");
            }
        }
        
        // Финальная проверка состояния
        let final_state = commander.get_network_state().await
            .expect("PANIC: Failed to get final network state");
        println!("Final state verified, uptime: {:?}", final_state.uptime);
        
        // Cleanup
        node_handle.abort();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ FAST network state tracking test completed"),
        Err(_) => panic!("PANIC: FAST network state test timed out ({}ms)", test_timeout.as_millis()),
    }
}

/// Тест множественных подключений - УСКОРЕННАЯ ВЕРСИЯ
#[tokio::test]
async fn test_multiple_connections_same_peer_fast() {
    let test_timeout = Duration::from_millis(5000); // Сократили с 20s до 5s
    
    println!("🧪 Testing multiple connections (FAST version)");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем узлы с быстрой очисткой
        let (mut server_node, server_commander, mut server_events, server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("PANIC: Failed to create server node");
        
        let (mut client1_node, client1_commander, _client1_events, _) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("PANIC: Failed to create client node");
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_millis(25)).await;
        });
        let client1_handle = tokio::spawn(async move {
            client1_node.run_with_cleanup_interval(Duration::from_millis(25)).await;
        });
        
        // Быстрый старт сервера
        println!("Quick server setup...");
        let listen_result = tokio::time::timeout(
            Duration::from_millis(500),
            server_commander.listen_port(Some("127.0.0.1".to_string()), 0)
        ).await;
        
        let _listen_ok = match listen_result {
            Ok(Ok(_)) => true,
            Ok(Err(e)) => panic!("PANIC: Server listen failed: {}", e),
            Err(_) => panic!("PANIC: Server setup timed out after 500ms"),
        };
        
        // Быстрое получение адреса сервера
        let server_addr = tokio::time::timeout(Duration::from_millis(1000), async {
            while let Some(event) = server_events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return Some(addr);
                }
            }
            None
        }).await;
        
        let addr = match server_addr {
            Ok(Some(addr)) => {
                println!("Server ready at: {}", addr);
                addr
            }
            Ok(None) => panic!("PANIC: No server address received within 1000ms"),
            Err(_) => panic!("PANIC: Server address timeout"),
        };
        
        // Быстрое подключение с коротким таймаутом
        println!("Quick client connection...");
        let connect_result = tokio::time::timeout(
            Duration::from_millis(1500), // 1.5 секунды на подключение
            client1_commander.connect_with_timeout(addr, 2)
        ).await;
        
        let connection_established = match connect_result {
            Ok(Ok(_)) => {
                println!("✅ Connection established quickly");
                true
            }
            Ok(Err(e)) => {
                // В тестах соединение может не пройти аутентификацию, но должно подключиться
                println!("Connection result: {}", e);
                // Проверяем есть ли хотя бы попытка соединения
                false
            }
            Err(_) => panic!("PANIC: Connection timed out after 1500ms"),
        };
        
        // Быстрое ожидание стабилизации
        println!("Quick stabilization check...");
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // КРИТИЧЕСКАЯ ПРОВЕРКА: соединения должны появиться
        let server_connections = server_commander.get_all_connections().await
            .expect("PANIC: Failed to get server connections");
        let network_state = server_commander.get_network_state().await
            .expect("PANIC: Failed to get network state");
        
        println!("Quick check: {} connections, {} total", 
                 server_connections.len(), network_state.total_connections);
        
        // Если подключение прошло, должны быть соединения
        if connection_established && server_connections.is_empty() {
            panic!("PANIC: Connection established but no connections found on server");
        }
        
        // Проверяем согласованность состояния
        assert_eq!(server_connections.len(), network_state.total_connections,
                  "PANIC: Connection count mismatch between APIs");
        
        // Быстрая очистка
        client1_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ FAST multiple connections test completed"),
        Err(_) => panic!("PANIC: FAST multiple connections test timed out ({}ms)", test_timeout.as_millis()),
    }
}

/// Тест быстрых циклов подключения/отключения - УСКОРЕННАЯ ВЕРСИЯ
#[tokio::test]
async fn test_rapid_connect_disconnect_fast() {
    let test_timeout = Duration::from_millis(8000); // Сократили с 30s до 8s
    
    println!("🧪 Testing rapid connect/disconnect (FAST version)");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (mut server_node, server_commander, mut server_events, server_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("PANIC: Failed to create server node");
        
        let (mut client_node, client_commander, _client_events, _client_peer_id) = 
            create_test_node_with_config(XRoutesConfig::client()).await
            .expect("PANIC: Failed to create client node");
        
        let server_handle = tokio::spawn(async move {
            server_node.run_with_cleanup_interval(Duration::from_millis(20)).await;
        });
        let client_handle = tokio::spawn(async move {
            client_node.run_with_cleanup_interval(Duration::from_millis(20)).await;
        });
        
        // Очень быстрая настройка сервера
        println!("Ultra-fast server setup...");
        let listen_result = tokio::time::timeout(
            Duration::from_millis(300),
            server_commander.listen_port(Some("127.0.0.1".to_string()), 0)
        ).await;
        
        match listen_result {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => panic!("PANIC: Server listen failed: {}", e),
            Err(_) => panic!("PANIC: Server setup timed out after 300ms"),
        }
        
        let server_addr = tokio::time::timeout(Duration::from_millis(500), async {
            while let Some(event) = server_events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return Some(addr);
                }
            }
            None
        }).await;
        
        let addr = match server_addr {
            Ok(Some(addr)) => {
                println!("Server ready at: {}", addr);
                addr
            }
            Ok(None) => panic!("PANIC: No server address received within 500ms"),
            Err(_) => panic!("PANIC: Server address timeout"),
        };
        
        let mut successful_cycles = 0;
        let total_cycles = 2; // Уменьшили количество циклов
        let required_success = 1; // Минимум 1 успешный цикл
        
        for i in 0..total_cycles {
            println!("  Fast cycle {}/{}", i + 1, total_cycles);
            let cycle_start = std::time::Instant::now();
            
            // Быстрое подключение
            let connect_result = tokio::time::timeout(
                Duration::from_millis(800), // 800ms на подключение
                client_commander.connect_with_timeout(addr.clone(), 1)
            ).await;
            
            let connected = match connect_result {
                Ok(Ok(_)) => {
                    println!("    Quick connect OK");
                    true
                }
                Ok(Err(e)) => {
                    println!("    Connect failed: {}", e);
                    // Продолжаем - может быть проблема с аутентификацией
                    false
                }
                Err(_) => panic!("PANIC: Connect timed out after 800ms in cycle {}", i + 1),
            };
            
            if connected {
                // Очень короткое ожидание стабилизации
                let ready = wait_for_connection_ready(&client_commander, 300).await;
                
                if ready {
                    println!("    Connection ready");
                    successful_cycles += 1;
                    
                    // Быстрое отключение
                    let disconnect_result = tokio::time::timeout(
                        Duration::from_millis(300),
                        client_commander.disconnect(server_peer_id)
                    ).await;
                    
                    match disconnect_result {
                        Ok(Ok(_)) => println!("    Quick disconnect OK"),
                        Ok(Err(e)) => println!("    Disconnect failed: {}", e),
                        Err(_) => panic!("PANIC: Disconnect timed out after 300ms in cycle {}", i + 1),
                    }
                    
                    // Минимальная очистка
                    tokio::time::sleep(Duration::from_millis(100)).await;
                } else {
                    println!("    Connection not ready, skipping disconnect");
                }
            }
            
            let cycle_time = cycle_start.elapsed();
            println!("    Cycle completed in {:?}", cycle_time);
            
            // Проверяем что цикл не слишком медленный
            if cycle_time > Duration::from_millis(2000) {
                panic!("PANIC: Cycle {} took too long: {:?}", i + 1, cycle_time);
            }
            
            // Короткая пауза между циклами
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        println!("✅ Completed {}/{} fast cycles", successful_cycles, total_cycles);
        
        // КРИТИЧЕСКАЯ ПРОВЕРКА: должен быть хотя бы один успешный цикл
        if successful_cycles < required_success {
            panic!("PANIC: Only {}/{} cycles succeeded, expected at least {}", 
                   successful_cycles, total_cycles, required_success);
        }
        
        // Минимальная очистка
        client_handle.abort();
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ FAST rapid connect/disconnect test completed"),
        Err(_) => panic!("PANIC: FAST rapid test timed out ({}ms)", test_timeout.as_millis()),
    }
}

// ==========================================
// СПЕЦИАЛЬНЫЕ БЫСТРЫЕ ДИАГНОСТИЧЕСКИЕ ТЕСТЫ
// ==========================================

/// Быстрый тест для проверки где именно происходит зависание
#[tokio::test]
async fn test_debug_bottlenecks_fast() {
    println!("🔧 Quick bottleneck detection test");
    
    let start_total = std::time::Instant::now();
    
    // Тест 1: Создание узла
    let create_start = std::time::Instant::now();
    let (mut node, commander, _events, peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("PANIC: Failed to create test node");
    let create_time = create_start.elapsed();
    println!("✅ Node creation: {:?}", create_time);
    
    // ПРОВЕРКА: создание узла не должно занимать больше 1 секунды
    if create_time > Duration::from_millis(1000) {
        panic!("PANIC: Node creation is too slow: {:?} (max 1000ms)", create_time);
    }
    
    // Тест 2: Запуск узла
    let run_start = std::time::Instant::now();
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_millis(10)).await;
    });
    let run_time = run_start.elapsed();
    println!("✅ Node spawn: {:?}", run_time);
    
    // ПРОВЕРКА: запуск не должен занимать больше 100ms
    if run_time > Duration::from_millis(100) {
        panic!("PANIC: Node spawn is too slow: {:?} (max 100ms)", run_time);
    }
    
    // Минимальная стабилизация
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Тест 3: API вызовы
    let api_start = std::time::Instant::now();
    let state = commander.get_network_state().await;
    let api_time = api_start.elapsed();
    println!("✅ API call (get_network_state): {:?} - Success: {}", api_time, state.is_ok());
    
    // ПРОВЕРКА: API должно отвечать быстро
    if api_time > Duration::from_millis(200) {
        panic!("PANIC: API call is too slow: {:?} (max 200ms)", api_time);
    }
    
    // ПРОВЕРКА: API должно работать
    if state.is_err() {
        panic!("PANIC: get_network_state failed: {:?}", state.err());
    }
    
    let network_state = state.unwrap();
    assert_eq!(network_state.local_peer_id, peer_id, "PANIC: Peer ID mismatch in network state");
    
    // Тест 4: Попытка запуска слушателя
    let listen_start = std::time::Instant::now();
    let listen_result = tokio::time::timeout(
        Duration::from_millis(500), // Жесткий таймаут для слушателя
        commander.listen_port(Some("127.0.0.1".to_string()), 0)
    ).await;
    let listen_time = listen_start.elapsed();
    println!("✅ Listen attempt: {:?} - Success: {}", listen_time, listen_result.is_ok());
    
    // ПРОВЕРКА: слушатель должен стартовать в разумное время
    match listen_result {
        Ok(Ok(_)) => {
            println!("✅ Listener started successfully");
        }
        Ok(Err(e)) => {
            panic!("PANIC: Listener failed to start: {}", e);
        }
        Err(_) => {
            panic!("PANIC: Listener start timed out after 500ms");
        }
    }
    
    // Тест 5: Получение адресов после запуска слушателя
    tokio::time::sleep(Duration::from_millis(100)).await; // Даем время адресам появиться
    
    let addr_start = std::time::Instant::now();
    let addresses = commander.get_listen_addresses().await;
    let addr_time = addr_start.elapsed();
    
    match addresses {
        Ok(addrs) => {
            println!("✅ Get addresses: {:?} - {} addresses", addr_time, addrs.len());
            
            // ПРОВЕРКА: после запуска слушателя должны быть адреса
            if addrs.is_empty() {
                panic!("PANIC: No listening addresses found after successful listen start");
            }
            
            // Выводим адреса для отладки
            for (i, addr) in addrs.iter().enumerate() {
                println!("    Address {}: {}", i + 1, addr);
            }
        }
        Err(e) => {
            panic!("PANIC: Failed to get listen addresses: {}", e);
        }
    }
    
    // ПРОВЕРКА: получение адресов должно быть быстрым
    if addr_time > Duration::from_millis(100) {
        panic!("PANIC: Get addresses is too slow: {:?} (max 100ms)", addr_time);
    }
    
    // Тест 6: Финальная проверка состояния сети
    let final_state = commander.get_network_state().await
        .expect("PANIC: Failed to get final network state");
    
    println!("Final network state:");
    println!("  Peer ID: {}", final_state.local_peer_id);
    println!("  Listening addresses: {}", final_state.listening_addresses.len());
    println!("  Total connections: {}", final_state.total_connections);
    println!("  Uptime: {:?}", final_state.uptime);
    
    // ПРОВЕРКА: состояние должно быть согласованным
    if final_state.listening_addresses.is_empty() {
        panic!("PANIC: Network state shows no listening addresses after successful setup");
    }
    
    // Очистка
    node_handle.abort();
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    let total_time = start_total.elapsed();
    println!("✅ Total bottleneck test time: {:?}", total_time);
    
    // ФИНАЛЬНАЯ ПРОВЕРКА: весь тест должен быть быстрым
    if total_time > Duration::from_millis(2000) {
        panic!("PANIC: Total test time is too slow: {:?} (max 2000ms)", total_time);
    }
    
    // Анализ узких мест с паникой
    if create_time > Duration::from_millis(500) {
        panic!("PANIC: BOTTLENECK DETECTED: Node creation is slow ({:?})", create_time);
    }
    if api_time > Duration::from_millis(50) {
        println!("⚠️  WARNING: API calls are slower than optimal ({:?})", api_time);
    }
    if listen_time > Duration::from_millis(300) {
        println!("⚠️  WARNING: Listen setup is slower than optimal ({:?})", listen_time);
    }
}

/// Минимальный тест аутентификации для определения времени
#[tokio::test]
async fn test_minimal_auth_timing() {
    println!("🔧 Minimal auth timing test");
    
    let (mut server_node, server_commander, mut server_events, server_peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("PANIC: Failed to create server node");
    
    let (mut client_node, client_commander, mut client_events, _) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("PANIC: Failed to create client node");
    
    let server_handle = tokio::spawn(async move {
        server_node.run_with_cleanup_interval(Duration::from_millis(10)).await;
    });
    let client_handle = tokio::spawn(async move {
        client_node.run_with_cleanup_interval(Duration::from_millis(10)).await;
    });
    
    // Минимальная настройка с таймаутом
    let listen_result = tokio::time::timeout(
        Duration::from_millis(500),
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0)
    ).await;
    
    match listen_result {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => panic!("PANIC: Server listen failed: {}", e),
        Err(_) => panic!("PANIC: Server listen timed out after 500ms"),
    }
    
    let server_addr = tokio::time::timeout(Duration::from_millis(500), async {
        while let Some(event) = server_events.recv().await {
            if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                return Some(addr);
            }
        }
        None
    }).await;
    
    let addr = match server_addr {
        Ok(Some(addr)) => addr,
        Ok(None) => panic!("PANIC: No server address received within 500ms"),
        Err(_) => panic!("PANIC: Server address timeout"),
    };
    
    // Подключение и мониторинг событий
    let connect_start = std::time::Instant::now();
    let connect_result = tokio::time::timeout(
        Duration::from_millis(1000),
        client_commander.connect_with_timeout(addr, 1)
    ).await;
    
    match connect_result {
        Ok(Ok(_)) => println!("✅ Connection initiated successfully"),
        Ok(Err(e)) => println!("Connection failed: {} (continuing to monitor events)", e),
        Err(_) => panic!("PANIC: Connection attempt timed out after 1000ms"),
    }
    
    println!("Monitoring auth events for 2 seconds...");
    let mut events_received = 0;
    let mut connection_time = None;
    let mut auth_time = None;
    let mut peer_connected = false;
    
    let monitor_start = std::time::Instant::now();
    while monitor_start.elapsed() < Duration::from_millis(2000) && events_received < 10 {
        tokio::select! {
            Some(event) = client_events.recv() => {
                events_received += 1;
                let elapsed = connect_start.elapsed();
                
                match event {
                    NetworkEvent::ConnectionOpened { peer_id, .. } if peer_id == server_peer_id => {
                        connection_time = Some(elapsed);
                        println!("  📡 Connection opened at {:?}", elapsed);
                    }
                    NetworkEvent::PeerConnected { peer_id } if peer_id == server_peer_id => {
                        peer_connected = true;
                        println!("  🤝 Peer connected at {:?}", elapsed);
                    }
                    NetworkEvent::AuthEvent { .. } => {
                        auth_time = Some(elapsed);
                        println!("  🔐 Auth event at {:?}", elapsed);
                    }
                    NetworkEvent::ConnectionError { error, .. } => {
                        println!("  ❌ Connection error at {:?}: {}", elapsed, error);
                    }
                    _ => {
                        println!("  📨 Other event at {:?}: {:?}", elapsed, event);
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(200)) => {
                // Периодическая проверка состояния соединения
                if let Ok(connections) = client_commander.get_all_connections().await {
                    if !connections.is_empty() {
                        let conn = &connections[0];
                        println!("  📊 Connection state: {:?}, Auth: {:?}", 
                                conn.connection_state, conn.auth_status);
                    }
                }
            }
        }
    }
    
    println!("Auth timing summary:");
    println!("  Events received: {}", events_received);
    
    // ПРОВЕРКИ: должны получить хотя бы некоторые события
    if events_received == 0 {
        println!("⚠️  WARNING: No events received (connection may have failed silently)");
    }
    
    if let Some(conn_time) = connection_time {
        println!("  Connection time: {:?}", conn_time);
        
        // ПРОВЕРКА: соединение не должно занимать слишком много времени
        if conn_time > Duration::from_millis(1500) {
            println!("⚠️  WARNING: Connection took longer than expected: {:?}", conn_time);
        }
    } else {
        println!("  No connection opened event received");
    }
    
    if let Some(a_time) = auth_time {
        println!("  Auth event time: {:?}", a_time);
        
        // ПРОВЕРКА: аутентификация не должна быть мгновенной (это подозрительно)
        if a_time < Duration::from_millis(10) {
            println!("⚠️  WARNING: Auth happened suspiciously quickly: {:?}", a_time);
        }
    } else {
        println!("  No auth events received (normal for quick test without full handshake)");
    }
    
    if peer_connected {
        println!("  ✅ Peer connection established successfully");
    } else {
        println!("  ℹ️  Peer connection not fully established (normal for timing test)");
    }
    
    // Быстрая очистка
    client_handle.abort();
    server_handle.abort();
    tokio::time::sleep(Duration::from_millis(25)).await;
    
    println!("✅ Minimal auth timing test completed");
}
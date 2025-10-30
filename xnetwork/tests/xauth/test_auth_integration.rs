//! Эталонный тест для XAuth интеграции в XNetwork
//! 
//! Этот тест демонстрирует правильную последовательность операций для тестирования NetCom нод:
//! 1. Создание нод
//! 2. Создание обработчиков событий  
//! 3. Запуск обработки событий
//! 4. Операции с командой
//! 5. Ожидание событий с таймаутами
//! 6. Проверки статуса аутентификации
//! 7. Корректное завершение

use std::time::Duration;

use crate::utils::node::create_node;
use crate::utils::event_handlers::{
    create_peer_connected_handler, 
    create_mutual_auth_handler,
    create_handler_submit_por_verification,
    create_listening_address_handler
};

#[tokio::test]
async fn test_auth_integration_basic() {
    println!("🧪 Testing basic XAuth integration in XNetwork");
    
    let test_timeout = Duration::from_secs(15);
    
    let result = tokio::time::timeout(test_timeout, async {
        // ✅ ПРАВИЛЬНО: сначала создаем ноды
        println!("🔄 Создаем серверную ноду...");
        let (server_commander, mut server_events, server_handle, server_peer_id) = 
            create_node().await.expect("Failed to create server node");
        
        println!("🔄 Создаем клиентскую ноду...");
        let (client_commander, mut client_events, client_handle, client_peer_id) = 
            create_node().await.expect("Failed to create client node");
        
        println!("✅ Ноды созданы:");
        println!("   Сервер: {:?}", server_peer_id);
        println!("   Клиент: {:?}", client_peer_id);
        
        // ✅ ПРАВИЛЬНО: создаем обработчики событий
        println!("🔄 Создаем обработчики событий...");
        
        // Обработчики для сервера
        let (server_listening_rx, mut server_listening_handler) = 
            create_listening_address_handler();
        let (server_connected_rx, mut server_connected_handler) = 
            create_peer_connected_handler(client_peer_id);
        let (server_auth_success_rx, mut server_auth_success_handler) = 
            create_mutual_auth_handler(client_peer_id);
        let mut server_por_approve_handler = 
            create_handler_submit_por_verification(server_commander.clone());
        
        // Обработчики для клиента
        let (client_connected_rx, mut client_connected_handler) = 
            create_peer_connected_handler(server_peer_id);
        let (client_auth_success_rx, mut client_auth_success_handler) = 
            create_mutual_auth_handler(server_peer_id);
        let mut client_por_approve_handler = 
            create_handler_submit_por_verification(client_commander.clone());
        
        // ✅ ПРАВИЛЬНО: сначала запускаем обработку событий
        println!("🔄 Запускаем обработку событий сервера...");
        let server_events_task = tokio::spawn(async move {
            while let Some(event) = server_events.recv().await {
                println!("📡 SERVER EVENT: {:?}", event);
                // обрабатывает событие прослушивания
                server_listening_handler(&event);
                // обрабатывает событие подключения
                server_connected_handler(&event);
                // обрабатывает событие авторизация пройдена
                server_auth_success_handler(&event);
                // обрабатывает событие por request, и всегда одобряет
                server_por_approve_handler(&event);
            }
        });
        
        println!("🔄 Запускаем обработку событий клиента...");
        let client_events_task = tokio::spawn(async move {
            while let Some(event) = client_events.recv().await {
                println!("📡 CLIENT EVENT: {:?}", event);
                client_connected_handler(&event);
                client_auth_success_handler(&event);
                client_por_approve_handler(&event);
            }
        });
        
        // ✅ ПРАВИЛЬНО: потом операции с командой
        
        // Сервер начинает слушать
        println!("🔄 Сервер начинает прослушивание...");
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await
            .expect("Failed to start server listening");
        
        // ✅ ПРАВИЛЬНО: ожидаем события прослушивания
        println!("⏳ Ожидаем события прослушивания...");
        let server_listening = tokio::time::timeout(Duration::from_secs(10), server_listening_rx).await
            .expect("Server should start listening within timeout")
            .expect("Failed to get listening address");
        
        println!("✅ Сервер запущен на адресе: {}", server_listening);
        
        let server_addr = server_listening;
        
        // Клиент подключается к серверу
        println!("🔄 Клиент подключается к серверу...");
        client_commander.connect(server_addr).await
            .expect("Failed to connect client to server");
        
        // ✅ ПРАВИЛЬНО: ожидаем события в теле теста
        println!("⏳ Ожидаем события подключения...");
        let client_connected = tokio::time::timeout(Duration::from_secs(10), client_connected_rx).await;
        let server_connected = tokio::time::timeout(Duration::from_secs(10), server_connected_rx).await;
        
        assert!(client_connected.is_ok(), "Client should connect to server");
        assert!(server_connected.is_ok(), "Server should see client connection");
        
        println!("✅ Оба узла получили события подключения");
        
        // ✅ ПРАВИЛЬНО: ожидаем события аутентификации
        println!("⏳ Ожидаем успешной аутентификации...");
        let client_auth_success = tokio::time::timeout(Duration::from_secs(10), client_auth_success_rx).await;
        let server_auth_success = tokio::time::timeout(Duration::from_secs(10), server_auth_success_rx).await;
        
        assert!(client_auth_success.is_ok(), "Client should complete authentication with server");
        assert!(server_auth_success.is_ok(), "Server should complete authentication with client");
        
        println!("✅ Оба узла завершили взаимную аутентификацию");
        
        // Проверяем статус аутентификации
        let is_client_authenticated = client_commander
            .is_peer_authenticated(server_peer_id).await
            .expect("Failed to check client authentication");
        
        let is_server_authenticated = server_commander
            .is_peer_authenticated(client_peer_id).await
            .expect("Failed to check server authentication");
        
        println!("🔐 СТАТУС АУТЕНТИФИКАЦИИ:");
        println!("   Client → Server: {}", is_client_authenticated);
        println!("   Server → Client: {}", is_server_authenticated);
        
        assert!(is_client_authenticated, "Client should be authenticated to server");
        assert!(is_server_authenticated, "Server should be authenticated to client");
        
        // Очистка
        println!("🔄 Завершаем работу нод...");
        client_commander.shutdown().await.expect("Failed to shutdown client");
        server_commander.shutdown().await.expect("Failed to shutdown server");
        
        // Ждем завершения задач
        let _ = tokio::join!(server_handle, client_handle, server_events_task, client_events_task);
        
        println!("✅ XAuth integration test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ XAuth integration test completed successfully"),
        Ok(Err(e)) => panic!("❌ XAuth integration test failed: {}", e),
        Err(_) => panic!("⏰ XAuth integration test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_auth_integration_multiple_connections() {
    println!("🧪 Testing XAuth integration with multiple connections");
    
    let test_timeout = Duration::from_secs(30);
    
    let result = tokio::time::timeout(test_timeout, async {
        // ✅ ПРАВИЛЬНО: сначала создаем ноды
        println!("🔄 Создаем серверную ноду...");
        let (server_commander, mut server_events, server_handle, server_peer_id) = 
            create_node().await.expect("Failed to create server node");
        
        println!("🔄 Создаем клиентскую ноду 1...");
        let (client1_commander, mut client1_events, client1_handle, client1_peer_id) = 
            create_node().await.expect("Failed to create client1 node");
        
        println!("🔄 Создаем клиентскую ноду 2...");
        let (client2_commander, mut client2_events, client2_handle, client2_peer_id) = 
            create_node().await.expect("Failed to create client2 node");
        
        println!("✅ Ноды созданы:");
        println!("   Сервер: {:?}", server_peer_id);
        println!("   Клиент 1: {:?}", client1_peer_id);
        println!("   Клиент 2: {:?}", client2_peer_id);
        
        // ✅ ПРАВИЛЬНО: создаем обработчики событий для всех нод
        println!("🔄 Создаем обработчики событий...");
        
        // Обработчики для сервера
        let (server_listening_rx, mut server_listening_handler) = 
            create_listening_address_handler();
        let (server_client1_connected_rx, mut server_client1_connected_handler) = 
            create_peer_connected_handler(client1_peer_id);
        let (server_client2_connected_rx, mut server_client2_connected_handler) = 
            create_peer_connected_handler(client2_peer_id);
        let (server_client1_auth_rx, mut server_client1_auth_handler) = 
            create_mutual_auth_handler(client1_peer_id);
        let (server_client2_auth_rx, mut server_client2_auth_handler) = 
            create_mutual_auth_handler(client2_peer_id);
        let mut server_por_approve_handler = 
            create_handler_submit_por_verification(server_commander.clone());
        
        // Обработчики для клиента 1
        let (client1_connected_rx, mut client1_connected_handler) = 
            create_peer_connected_handler(server_peer_id);
        let (client1_auth_rx, mut client1_auth_handler) = 
            create_mutual_auth_handler(server_peer_id);
        let mut client1_por_approve_handler = 
            create_handler_submit_por_verification(client1_commander.clone());
        
        // Обработчики для клиента 2
        let (client2_connected_rx, mut client2_connected_handler) = 
            create_peer_connected_handler(server_peer_id);
        let (client2_auth_rx, mut client2_auth_handler) = 
            create_mutual_auth_handler(server_peer_id);
        let mut client2_por_approve_handler = 
            create_handler_submit_por_verification(client2_commander.clone());
        
        // ✅ ПРАВИЛЬНО: сначала запускаем обработку событий
        println!("🔄 Запускаем обработку событий сервера...");
        let server_events_task = tokio::spawn(async move {
            while let Some(event) = server_events.recv().await {
                println!("📡 SERVER EVENT: {:?}", event);
                server_listening_handler(&event);
                server_client1_connected_handler(&event);
                server_client2_connected_handler(&event);
                server_client1_auth_handler(&event);
                server_client2_auth_handler(&event);
                server_por_approve_handler(&event);
            }
        });
        
        println!("🔄 Запускаем обработку событий клиента 1...");
        let client1_events_task = tokio::spawn(async move {
            while let Some(event) = client1_events.recv().await {
                println!("📡 CLIENT1 EVENT: {:?}", event);
                client1_connected_handler(&event);
                client1_auth_handler(&event);
                client1_por_approve_handler(&event);
            }
        });
        
        println!("🔄 Запускаем обработку событий клиента 2...");
        let client2_events_task = tokio::spawn(async move {
            while let Some(event) = client2_events.recv().await {
                println!("📡 CLIENT2 EVENT: {:?}", event);
                client2_connected_handler(&event);
                client2_auth_handler(&event);
                client2_por_approve_handler(&event);
            }
        });
        
        // ✅ ПРАВИЛЬНО: потом операции с командой
        
        // Сервер начинает слушать
        println!("🔄 Сервер начинает прослушивание...");
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await
            .expect("Failed to start server listening");
        
        // ✅ ПРАВИЛЬНО: ожидаем события прослушивания
        println!("⏳ Ожидаем события прослушивания...");
        let server_listening = tokio::time::timeout(Duration::from_secs(10), server_listening_rx).await
            .expect("Server should start listening within timeout")
            .expect("Failed to get listening address");
        
        println!("✅ Сервер запущен на адресе: {}", server_listening);
        
        let server_addr = server_listening;
        
        // Клиенты подключаются к серверу
        println!("🔄 Клиент 1 подключается к серверу...");
        client1_commander.connect(server_addr.clone()).await
            .expect("Failed to connect client1 to server");
        
        println!("🔄 Клиент 2 подключается к серверу...");
        client2_commander.connect(server_addr).await
            .expect("Failed to connect client2 to server");
        
        // ✅ ПРАВИЛЬНО: ожидаем события подключения
        println!("⏳ Ожидаем события подключения...");
        let client1_connected = tokio::time::timeout(Duration::from_secs(10), client1_connected_rx).await;
        let client2_connected = tokio::time::timeout(Duration::from_secs(10), client2_connected_rx).await;
        let server_client1_connected = tokio::time::timeout(Duration::from_secs(10), server_client1_connected_rx).await;
        let server_client2_connected = tokio::time::timeout(Duration::from_secs(10), server_client2_connected_rx).await;
        
        assert!(client1_connected.is_ok(), "Client1 should connect to server");
        assert!(client2_connected.is_ok(), "Client2 should connect to server");
        assert!(server_client1_connected.is_ok(), "Server should see client1 connection");
        assert!(server_client2_connected.is_ok(), "Server should see client2 connection");
        
        println!("✅ Все узлы получили события подключения");
        
        // ✅ ПРАВИЛЬНО: ожидаем события аутентификации
        println!("⏳ Ожидаем успешной аутентификации...");
        let client1_auth_success = tokio::time::timeout(Duration::from_secs(10), client1_auth_rx).await;
        let client2_auth_success = tokio::time::timeout(Duration::from_secs(10), client2_auth_rx).await;
        let server_client1_auth_success = tokio::time::timeout(Duration::from_secs(10), server_client1_auth_rx).await;
        let server_client2_auth_success = tokio::time::timeout(Duration::from_secs(10), server_client2_auth_rx).await;
        
        assert!(client1_auth_success.is_ok(), "Client1 should complete authentication with server");
        assert!(client2_auth_success.is_ok(), "Client2 should complete authentication with server");
        assert!(server_client1_auth_success.is_ok(), "Server should complete authentication with client1");
        assert!(server_client2_auth_success.is_ok(), "Server should complete authentication with client2");
        
        println!("✅ Все узлы завершили взаимную аутентификацию");
        
        // Проверяем статус аутентификации
        let is_client1_authenticated = client1_commander
            .is_peer_authenticated(server_peer_id).await
            .expect("Failed to check client1 authentication");
        
        let is_client2_authenticated = client2_commander
            .is_peer_authenticated(server_peer_id).await
            .expect("Failed to check client2 authentication");
        
        let is_server_client1_authenticated = server_commander
            .is_peer_authenticated(client1_peer_id).await
            .expect("Failed to check server authentication for client1");
        
        let is_server_client2_authenticated = server_commander
            .is_peer_authenticated(client2_peer_id).await
            .expect("Failed to check server authentication for client2");
        
        println!("🔐 СТАТУС АУТЕНТИФИКАЦИИ:");
        println!("   Client1 → Server: {}", is_client1_authenticated);
        println!("   Client2 → Server: {}", is_client2_authenticated);
        println!("   Server → Client1: {}", is_server_client1_authenticated);
        println!("   Server → Client2: {}", is_server_client2_authenticated);
        
        assert!(is_client1_authenticated, "Client1 should be authenticated to server");
        assert!(is_client2_authenticated, "Client2 should be authenticated to server");
        assert!(is_server_client1_authenticated, "Server should be authenticated to client1");
        assert!(is_server_client2_authenticated, "Server should be authenticated to client2");
        
        // Очистка
        println!("🔄 Завершаем работу нод...");
        client1_commander.shutdown().await.expect("Failed to shutdown client1");
        client2_commander.shutdown().await.expect("Failed to shutdown client2");
        server_commander.shutdown().await.expect("Failed to shutdown server");
        
        // Ждем завершения задач
        let _ = tokio::join!(server_handle, client1_handle, client2_handle, server_events_task, client1_events_task, client2_events_task);
        
        println!("✅ Multiple connections test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ Multiple connections test completed successfully"),
        Ok(Err(e)) => panic!("❌ Multiple connections test failed: {}", e),
        Err(_) => panic!("⏰ Multiple connections test timed out ({}s)", test_timeout.as_secs()),
    }
}

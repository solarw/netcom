//! Тест взаимной XAuth аутентификации двух нод

use std::time::Duration;
use tokio::time::timeout;
use xnetwork2::Node;
use xnetwork2::node_events::NodeEvent;

/// Утилита для ожидания конкретного события с таймаутом
async fn wait_for_event<F>(
    events: &mut tokio::sync::broadcast::Receiver<NodeEvent>,
    predicate: F,
    timeout_duration: Duration,
) -> Result<NodeEvent, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(&NodeEvent) -> bool,
{
    timeout(timeout_duration, async {
        loop {
            match events.recv().await {
                Ok(event) => {
                    if predicate(&event) {
                        return Ok(event);
                    }
                }
                Err(e) => {
                    return Err(format!("❌ Ошибка получения события: {} - система событий не работает", e).into());
                }
            }
        }
    }).await?
}

/// Утилита для ожидания двух событий в неизвестном порядке
async fn wait_for_two_events<F1, F2>(
    events1: &mut tokio::sync::broadcast::Receiver<NodeEvent>,
    events2: &mut tokio::sync::broadcast::Receiver<NodeEvent>,
    predicate1: F1,
    predicate2: F2,
    timeout_duration: Duration,
) -> Result<(NodeEvent, NodeEvent), Box<dyn std::error::Error + Send + Sync>>
where
    F1: Fn(&NodeEvent) -> bool,
    F2: Fn(&NodeEvent) -> bool,
{
    timeout(timeout_duration, async {
        let mut event1_opt = None;
        let mut event2_opt = None;
        
        while event1_opt.is_none() || event2_opt.is_none() {
            tokio::select! {
                Ok(event) = events1.recv() => {
                    if predicate1(&event) && event1_opt.is_none() {
                        event1_opt = Some(event);
                    }
                }
                Ok(event) = events2.recv() => {
                    if predicate2(&event) && event2_opt.is_none() {
                        event2_opt = Some(event);
                    }
                }
            }
        }
        
        Ok((event1_opt.unwrap(), event2_opt.unwrap()))
    }).await?
}

/// Тестирует взаимную XAuth аутентификацию двух нод
/// Весь тест должен укладываться в 5 секунд
/// Любая ошибка аутентификации или таймаут - паника
#[tokio::test]
async fn test_two_nodes_xauth_mutual_authentication_in_5_seconds() {
    println!("🧪 Запуск теста взаимной XAuth аутентификации (5 секунд)...");
    
    // Таймаут на весь тест - 5 секунд
    let result = timeout(Duration::from_secs(5), async {
        // 1. СОЗДАНИЕ ДВУХ НОД (0-1 секунда)
        println!("🆕 Создаем две ноды...");
        let mut node1 = Node::new().await
            .expect("❌ Не удалось создать первую ноду - критическая ошибка");
        let mut node2 = Node::new().await
            .expect("❌ Не удалось создать вторую ноду - критическая ошибка");
        
        println!("✅ Ноды созданы:");
        println!("   Node1 PeerId: {}", node1.peer_id());
        println!("   Node2 PeerId: {}", node2.peer_id());
        
        // 2. ПОДПИСКА НА СОБЫТИЯ ДО ЗАПУСКА
        println!("📡 Подписываемся на события обеих нод...");
        let mut node1_events = node1.subscribe();
        let mut node2_events = node2.subscribe();
        
        // 3. ЗАПУСК ОБЕИХ НОД (1-2 секунды)
        println!("🚀 Запускаем обе ноды...");
        node1.start().await
            .expect("❌ Не удалось запустить первую ноду - критическая ошибка");
        node2.start().await
            .expect("❌ Не удалось запустить вторую ноду - критическая ошибка");
        
        println!("✅ Обе ноды запущены:");
        println!("   Node1 состояние: {}", node1.get_task_status());
        println!("   Node2 состояние: {}", node2.get_task_status());
        
        // 4. НОДА1 НАЧИНАЕТ СЛУШАТЬ (2-3 секунды)
        println!("🎯 Нода1 начинает прослушивание...");
        node1.commander.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap()).await
            .expect("❌ Не удалось выполнить listen_on - критическая ошибка");
        
        println!("✅ Команда listen_on выполнена, ожидаем событие...");
        
        // 5. ОЖИДАНИЕ СОБЫТИЯ NewListenAddr НА НОДЕ1 (3-3.5 секунды)
        println!("⏳ Ожидаем событие NewListenAddr на ноде1 (таймаут 1 секунда)...");
        let listen_event = wait_for_event(
            &mut node1_events,
            |e| matches!(e, NodeEvent::NewListenAddr { .. }),
            Duration::from_secs(1)
        ).await.expect("❌ Таймаут ожидания события NewListenAddr - событие не пришло за 1 секунду");
        
        let listen_addr = match listen_event {
            NodeEvent::NewListenAddr { address } => address,
            _ => panic!("❌ Получено неожиданное событие: {:?}", listen_event),
        };
        
        println!("✅ Нода1 слушает на адресе: {}", listen_addr);
        
        // 6. НОДА2 ПОДКЛЮЧАЕТСЯ К НОДЕ1 (3.5-4 секунды)
        println!("🔗 Нода2 подключается к ноде1...");
        node2.commander.dial(node1.peer_id().clone(), listen_addr.clone()).await
            .expect("❌ Не удалось выполнить dial - критическая ошибка");
        
        println!("✅ Команда dial выполнена, ожидаем события подключения...");
        
        // 7. ОЖИДАНИЕ СОБЫТИЙ ConnectionEstablished НА ОБЕИХ НОДАХ (4-4.5 секунды)
        println!("⏳ Ожидаем события ConnectionEstablished на обеих нодах (таймаут 2 секунды)...");
        let (node1_connected, node2_connected) = wait_for_two_events(
            &mut node1_events,
            &mut node2_events,
            |e| matches!(e, NodeEvent::ConnectionEstablished { .. }),
            |e| matches!(e, NodeEvent::ConnectionEstablished { .. }),
            Duration::from_secs(2)
        ).await.expect("❌ Таймаут ожидания событий ConnectionEstablished - соединение не установлено за 2 секунды");
        
        // 8. ПРОВЕРКА ЦЕЛОСТНОСТИ СОЕДИНЕНИЯ (4.5-4.8 секунды)
        println!("🔍 Проверяем целостность соединения...");
        
        let node1_peer_id = match node1_connected {
            NodeEvent::ConnectionEstablished { peer_id } => peer_id,
            _ => panic!("❌ Нода1 получила неожиданное событие: {:?}", node1_connected),
        };
        
        let node2_peer_id = match node2_connected {
            NodeEvent::ConnectionEstablished { peer_id } => peer_id,
            _ => panic!("❌ Нода2 получила неожиданное событие: {:?}", node2_connected),
        };
        
        assert_eq!(node1_peer_id, *node2.peer_id(), 
            "❌ Нода1 видит подключение от неверного пира: {} вместо {}", 
            node1_peer_id, node2.peer_id());
        
        assert_eq!(node2_peer_id, *node1.peer_id(), 
            "❌ Нода2 видит подключение от неверного пира: {} вместо {}", 
            node2_peer_id, node1.peer_id());
        
        println!("✅ Соединение установлено корректно:");
        println!("   Node1 → Node2: {}", node1_peer_id);
        println!("   Node2 → Node1: {}", node2_peer_id);
        
        // 9. ОЖИДАНИЕ СОБЫТИЙ VerifyPorRequest НА ОБЕИХ НОДАХ (4.8-5.5 секунды)
        println!("🔐 Ожидаем события VerifyPorRequest на обеих нодах (таймаут 3 секунды)...");
        let (node1_por_request, node2_por_request) = wait_for_two_events(
            &mut node1_events,
            &mut node2_events,
            |e| matches!(e, NodeEvent::VerifyPorRequest { .. }),
            |e| matches!(e, NodeEvent::VerifyPorRequest { .. }),
            Duration::from_secs(3)
        ).await.expect("❌ Таймаут ожидания событий VerifyPorRequest - PoR запросы не пришли за 3 секунды");
        
        // 10. ОБРАБОТКА PoR ЗАПРОСОВ И ПОДТВЕРЖДЕНИЕ (5.5-6 секунды)
        println!("✅ Получены PoR запросы, подтверждаем аутентификацию...");
        
        // Обрабатываем PoR запросы и подтверждаем их
        match node1_por_request {
            NodeEvent::VerifyPorRequest { peer_id, connection_id, .. } => {
                println!("   Node1 подтверждает PoR для пира: {}", peer_id);
                // Используем новую команду submit_por_verification
                node1.commander.submit_por_verification(peer_id, true).await
                    .expect("❌ Не удалось подтвердить аутентификацию на ноде1 - критическая ошибка");
            }
            _ => panic!("❌ Нода1 получила неожиданное событие: {:?}", node1_por_request),
        }
        
        match node2_por_request {
            NodeEvent::VerifyPorRequest { peer_id, connection_id, .. } => {
                println!("   Node2 подтверждает PoR для пира: {}", peer_id);
                // Используем новую команду submit_por_verification
                node2.commander.submit_por_verification(peer_id, true).await
                    .expect("❌ Не удалось подтвердить аутентификацию на ноде2 - критическая ошибка");
            }
            _ => panic!("❌ Нода2 получила неожиданное событие: {:?}", node2_por_request),
        }
        
        println!("✅ PoR запросы подтверждены, ожидаем завершение аутентификации...");
        
        // 11. ОЖИДАНИЕ СОБЫТИЙ PeerAuthenticated (6-6.5 секунды)
        println!("⏳ Ожидаем события взаимной XAuth аутентификации (таймаут 2 секунды)...");
        let (node1_auth, node2_auth) = wait_for_two_events(
            &mut node1_events,
            &mut node2_events,
            |e| matches!(e, NodeEvent::PeerAuthenticated { .. }),
            |e| matches!(e, NodeEvent::PeerAuthenticated { .. }),
            Duration::from_secs(2)
        ).await.expect("❌ Таймаут ожидания событий PeerAuthenticated - аутентификация не завершена за 2 секунды");
        
        // 12. ПРОВЕРКА ЦЕЛОСТНОСТИ АУТЕНТИФИКАЦИИ (6.5-6.8 секунды)
        println!("🔍 Проверяем целостность аутентификации...");
        
        let node1_auth_peer_id = match node1_auth {
            NodeEvent::PeerAuthenticated { peer_id } => peer_id,
            _ => panic!("❌ Нода1 получила неожиданное событие аутентификации: {:?}", node1_auth),
        };
        
        let node2_auth_peer_id = match node2_auth {
            NodeEvent::PeerAuthenticated { peer_id } => peer_id,
            _ => panic!("❌ Нода2 получила неожиданное событие аутентификации: {:?}", node2_auth),
        };
        
        // Проверяем, что аутентификация прошла взаимно
        assert_eq!(node1_auth_peer_id, *node2.peer_id(), 
            "❌ Нода1 аутентифицировала неверного пира: {} вместо {}", 
            node1_auth_peer_id, node2.peer_id());
        
        assert_eq!(node2_auth_peer_id, *node1.peer_id(), 
            "❌ Нода2 аутентифицировала неверного пира: {} вместо {}", 
            node2_auth_peer_id, node1.peer_id());
        
        println!("✅ Взаимная XAuth аутентификация успешно завершена:");
        println!("   Node1 → Node2: {}", node1_auth_peer_id);
        println!("   Node2 → Node1: {}", node2_auth_peer_id);
        
        // 11. GRACEFUL SHUTDOWN ОБЕИХ НОД (5.2-5.5 секунд)
        println!("🛑 Выполняем graceful shutdown обеих нод...");
        node1.commander.shutdown().await
            .expect("❌ Не удалось выполнить graceful shutdown ноды1 - критическая ошибка");
        node2.commander.shutdown().await
            .expect("❌ Не удалось выполнить graceful shutdown ноды2 - критическая ошибка");
        
        println!("⏳ Ожидаем завершение фоновых задач...");
        node1.wait_for_shutdown().await
            .expect("❌ Не удалось дождаться завершения ноды1 - критическая ошибка");
        node2.wait_for_shutdown().await
            .expect("❌ Не удалось дождаться завершения ноды2 - критическая ошибка");
        
        println!("✅ Обе ноды корректно завершили работу");
        
        // 12. ФИНАЛЬНАЯ ПРОВЕРКА
        assert_eq!(node1.get_task_status(), "not_started", 
            "❌ Нода1 не перешла в состояние 'not_started' после завершения");
        assert_eq!(node2.get_task_status(), "not_started", 
            "❌ Нода2 не перешла в состояние 'not_started' после завершения");
        assert!(!node1.is_running(), "❌ Нода1 все еще работает после graceful shutdown");
        assert!(!node2.is_running(), "❌ Нода2 все еще работает после graceful shutdown");
        
        println!("🎉 Тест взаимной XAuth аутентификации успешно завершен!");
    }).await;
    
    // Проверяем, что тест уложился в 5 секунд
    match result {
        Ok(_) => println!("✅ Тест выполнен за {} секунд - ВСЕГО 5 СЕКУНД!", 5),
        Err(_) => panic!("❌ ТЕСТ ПРЕВЫСИЛ ЛИМИТ В 5 СЕКУНД - ПРОБЛЕМА ПРОИЗВОДИТЕЛЬНОСТИ!"),
    }
}

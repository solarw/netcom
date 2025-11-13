//! Тестирование методов listen_and_wait и dial_and_wait

use std::time::Duration;
use tokio::time::timeout;
use xnetwork2::Node;
use xnetwork2::node_events::NodeEvent;

mod utils;
use utils::wait_for_event;

/// Тестирует метод listen_and_wait - прослушивание с ожиданием адреса
#[tokio::test]
async fn test_listen_and_wait_in_5_seconds() {
    println!("🧪 Запуск теста listen_and_wait (5 секунд)...");

    let result = timeout(Duration::from_secs(5), async {
        // 1. СОЗДАНИЕ НОДЫ
        println!("🆕 Создаем ноду...");
        let mut node = Node::new().await
            .expect("❌ Не удалось создать ноду - критическая ошибка");

        println!("✅ Нода создана с PeerId: {}", node.peer_id());

        // 2. ПОДПИСКА НА СОБЫТИЯ
        println!("📡 Подписываемся на события ноды...");
        let mut events = node.subscribe();

        // 3. ЗАПУСК НОДЫ
        println!("🚀 Запускаем ноду...");
        node.start().await
            .expect("❌ Не удалось запустить ноду - критическая ошибка");

        println!("✅ Нода запущена, состояние: {}", node.get_task_status());

        // 4. ВЫПОЛНЕНИЕ LISTEN_AND_WAIT
        println!("🎯 Выполняем listen_and_wait...");
        let listen_addr = node.commander.listen_and_wait("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap(), Duration::from_secs(3)).await
            .expect("❌ Не удалось выполнить listen_and_wait - критическая ошибка");

        println!("✅ listen_and_wait выполнен успешно, адрес: {}", listen_addr);

        // 5. ПРОВЕРКА ЦЕЛОСТНОСТИ
        println!("🔍 Проверяем целостность...");

        // Проверяем, что адрес не пустой
        assert!(
            !listen_addr.to_string().is_empty(),
            "❌ Получен пустой адрес прослушивания"
        );

        // Проверяем формат адреса
        assert!(
            listen_addr.to_string().contains("/ip4/127.0.0.1/udp/"),
            "❌ Неверный формат адреса: {}",
            listen_addr
        );

        // Проверяем, что адрес содержит порт
        assert!(
            listen_addr.to_string().contains("/udp/"),
            "❌ Адрес не содержит порт: {}",
            listen_addr
        );

        // 6. ПРОВЕРКА СОБЫТИЯ
        println!("⏳ Проверяем, что событие NewListenAddr было получено...");
        let listen_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::NewListenAddr { .. }),
            Duration::from_secs(1)
        ).await.expect("❌ Событие NewListenAddr не получено");

        match listen_event {
            NodeEvent::NewListenAddr { address, listener_id: _ } => {
                assert_eq!(address, listen_addr, "❌ Адрес из события не совпадает с возвращенным");
                println!("✅ Событие NewListenAddr корректно получено: {}", address);
            }
            _ => panic!("❌ Получено неожиданное событие: {:?}", listen_event),
        }

        // 7. GRACEFUL SHUTDOWN
        println!("🛑 Выполняем graceful shutdown...");
        node.commander.shutdown().await
            .expect("❌ Не удалось выполнить graceful shutdown - критическая ошибка");

        node.wait_for_shutdown().await
            .expect("❌ Не удалось дождаться завершения ноды - критическая ошибка");

        println!("✅ Нода корректно завершила работу");

        // 8. ФИНАЛЬНАЯ ПРОВЕРКА
        assert_eq!(
            node.get_task_status(),
            "not_started",
            "❌ Нода не перешла в состояние 'not_started' после завершения"
        );
        assert!(
            !node.is_running(),
            "❌ Нода все еще работает после graceful shutdown"
        );

        println!("🎉 Тест listen_and_wait успешно завершен!");
    }).await;

    // Проверяем, что тест уложился в 5 секунд
    match result {
        Ok(_) => println!("✅ Тест выполнен за {} секунд - ВСЕГО 5 СЕКУНД!", 5),
        Err(_) => panic!("❌ ТЕСТ ПРЕВЫСИЛ ЛИМИТ В 5 СЕКУНД - ПРОБЛЕМА ПРОИЗВОДИТЕЛЬНОСТИ!"),
    }
}

/// Тестирует метод dial_and_wait - подключение с ожиданием соединения
#[tokio::test]
async fn test_dial_and_wait_in_5_seconds() {
    println!("🧪 Запуск теста dial_and_wait (5 секунд)...");

    let result = timeout(Duration::from_secs(5), async {
        // 1. СОЗДАНИЕ ДВУХ НОД
        println!("🆕 Создаем две ноды...");
        let mut node1 = Node::new().await
            .expect("❌ Не удалось создать первую ноду - критическая ошибка");
        let mut node2 = Node::new().await
            .expect("❌ Не удалось создать вторую ноду - критическая ошибка");

        println!("✅ Ноды созданы:");
        println!("   Node1 PeerId: {}", node1.peer_id());
        println!("   Node2 PeerId: {}", node2.peer_id());

        // 2. ПОДПИСКА НА СОБЫТИЯ
        println!("📡 Подписываемся на события обеих нод...");
        let mut node1_events = node1.subscribe();
        let mut node2_events = node2.subscribe();

        // 3. ЗАПУСК ОБЕИХ НОД
        println!("🚀 Запускаем обе ноды...");
        node1.start().await
            .expect("❌ Не удалось запустить первую ноду - критическая ошибка");
        node2.start().await
            .expect("❌ Не удалось запустить вторую ноду - критическая ошибка");

        println!("✅ Обе ноды запущены:");
        println!("   Node1 состояние: {}", node1.get_task_status());
        println!("   Node2 состояние: {}", node2.get_task_status());

        // 4. НОДА1 НАЧИНАЕТ СЛУШАТЬ ЧЕРЕЗ LISTEN_AND_WAIT
        println!("🎯 Нода1 начинает прослушивание через listen_and_wait...");
        let listen_addr = node1.commander.listen_and_wait("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap(), Duration::from_secs(3)).await
            .expect("❌ Не удалось выполнить listen_and_wait - критическая ошибка");

        println!("✅ Нода1 слушает на адресе: {}", listen_addr);

        // 5. НОДА2 ПОДКЛЮЧАЕТСЯ ЧЕРЕЗ DIAL_AND_WAIT
        println!("🔗 Нода2 подключается к ноде1 через dial_and_wait...");
        node2.commander.dial_and_wait(node1.peer_id().clone(), listen_addr.clone(), Duration::from_secs(3)).await
            .expect("❌ Не удалось выполнить dial_and_wait - критическая ошибка");

        println!("✅ dial_and_wait выполнен успешно");

        // 6. ПРОВЕРКА СОБЫТИЙ CONNECTION_ESTABLISHED
        println!("⏳ Проверяем события ConnectionEstablished на обеих нодах...");

        // Ожидаем ConnectionEstablished на ноде1
        let node1_connected = wait_for_event(
            &mut node1_events,
            |e| matches!(e, NodeEvent::ConnectionEstablished { .. }),
            Duration::from_secs(2)
        ).await.expect("❌ Нода1 не получила ConnectionEstablished");

        // Ожидаем ConnectionEstablished на ноде2
        let node2_connected = wait_for_event(
            &mut node2_events,
            |e| matches!(e, NodeEvent::ConnectionEstablished { .. }),
            Duration::from_secs(2)
        ).await.expect("❌ Нода2 не получила ConnectionEstablished");

        // 7. ПРОВЕРКА ЦЕЛОСТНОСТИ СОЕДИНЕНИЯ
        println!("🔍 Проверяем целостность соединения...");

        let node1_peer_id = match node1_connected {
            NodeEvent::ConnectionEstablished { peer_id, connection_id: _ } => peer_id,
            _ => panic!("❌ Нода1 получила неожиданное событие: {:?}", node1_connected),
        };

        let node2_peer_id = match node2_connected {
            NodeEvent::ConnectionEstablished { peer_id, connection_id: _ } => peer_id,
            _ => panic!("❌ Нода2 получила неожиданное событие: {:?}", node2_connected),
        };

        // Проверяем, что ноды видят друг друга
        assert_eq!(node1_peer_id, *node2.peer_id(),
            "❌ Нода1 видит подключение от неверного пира: {} вместо {}",
            node1_peer_id, node2.peer_id());

        assert_eq!(node2_peer_id, *node1.peer_id(),
            "❌ Нода2 видит подключение от неверного пира: {} вместо {}",
            node2_peer_id, node1.peer_id());

        println!("✅ Соединение установлено корректно:");
        println!("   Node1 → Node2: {}", node1_peer_id);
        println!("   Node2 → Node1: {}", node2_peer_id);

        // 8. GRACEFUL SHUTDOWN ОБЕИХ НОД
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

        // 9. ФИНАЛЬНАЯ ПРОВЕРКА
        assert_eq!(node1.get_task_status(), "not_started",
            "❌ Нода1 не перешла в состояние 'not_started' после завершения");
        assert_eq!(node2.get_task_status(), "not_started",
            "❌ Нода2 не перешла в состояние 'not_started' после завершения");
        assert!(!node1.is_running(), "❌ Нода1 все еще работает после graceful shutdown");
        assert!(!node2.is_running(), "❌ Нода2 все еще работает после graceful shutdown");

        println!("🎉 Тест dial_and_wait успешно завершен!");
    }).await;

    // Проверяем, что тест уложился в 5 секунд
    match result {
        Ok(_) => println!("✅ Тест выполнен за {} секунд - ВСЕГО 5 СЕКУНД!", 5),
        Err(_) => panic!("❌ ТЕСТ ПРЕВЫСИЛ ЛИМИТ В 5 СЕКУНД - ПРОБЛЕМА ПРОИЗВОДИТЕЛЬНОСТИ!"),
    }
}

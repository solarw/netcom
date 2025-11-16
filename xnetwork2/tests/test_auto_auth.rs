//! Тест автоматической аутентификации XAuth

use std::time::Duration;
use tokio::time::timeout;
use xnetwork2::{Node, node_builder::InboundDecisionPolicy};

mod utils;
use utils::{setup_listening_node, dial_and_wait_connection, spawn_auth_completion_task, spawn_auto_respond_por_task};

/// Тест автоматической аутентификации с AutoApprove политикой
#[tokio::test]
async fn test_auto_auth_with_auto_approve() {
    println!("🧪 Тестирование автоматической аутентификации с AutoApprove...");

    let result = timeout(Duration::from_secs(10), async {
        // Создаем ноды с автоматической политикой
        println!("🆕 Создаем две ноды с AutoApprove политикой...");
        let mut node1 = xnetwork2::node_builder::NodeBuilder::new()
            .with_inbound_decision_policy(InboundDecisionPolicy::AutoApprove)
            .build()
            .await
            .expect("❌ Не удалось создать ноду 1");
        
        let mut node2 = xnetwork2::node_builder::NodeBuilder::new()
            .with_inbound_decision_policy(InboundDecisionPolicy::AutoApprove)
            .build()
            .await
            .expect("❌ Не удалось создать ноду 2");

        println!("✅ Ноды созданы:");
        println!("   Node1 PeerId: {}", node1.peer_id());
        println!("   Node2 PeerId: {}", node2.peer_id());

        // Запускаем ноды
        println!("🚀 Запускаем обе ноды...");
        node1.start().await.expect("❌ Не удалось запустить ноду 1");
        node2.start().await.expect("❌ Не удалось запустить ноду 2");

        // Настраиваем прослушивание
        println!("🎯 Настраиваем прослушивание...");
        let node1_addr = setup_listening_node(&mut node1).await
            .expect("❌ Не удалось настроить прослушивание ноды 1");
        let node2_addr = setup_listening_node(&mut node2).await
            .expect("❌ Не удалось настроить прослушивание ноды 2");

        println!("✅ Ноды слушают:");
        println!("   Node1 адрес: {}", node1_addr);
        println!("   Node2 адрес: {}", node2_addr);

        // Устанавливаем автоматический режим аутентификации для обеих нод
        println!("🔄 Устанавливаем автоматический режим аутентификации для обеих нод...");
        node1.commander.set_auto_auth_mode(true).await
            .expect("❌ Не удалось установить автоматический режим для ноды 1");
        node2.commander.set_auto_auth_mode(true).await
            .expect("❌ Не удалось установить автоматический режим для ноды 2");

        // Запускаем задачи ожидания завершения аутентификации
        let auth_completion_task1 = spawn_auth_completion_task(&mut node1, *node2.peer_id(), Duration::from_secs(5));
        let auth_completion_task2 = spawn_auth_completion_task(&mut node2, *node1.peer_id(), Duration::from_secs(5));

        // Запускаем задачи автоматического ответа на PoR запросы
        let auto_respond_task1 = spawn_auto_respond_por_task(&mut node1, *node2.peer_id(), Duration::from_secs(5));
        let auto_respond_task2 = spawn_auto_respond_por_task(&mut node2, *node1.peer_id(), Duration::from_secs(5));

        // Устанавливаем соединение
        println!("🔗 Подключаем ноду 1 к ноде 2...");
        let _connection_id1 = dial_and_wait_connection(
            &mut node1, 
            *node2.peer_id(), 
            node2_addr.clone(), 
            Duration::from_secs(5)
        ).await.expect("❌ Не удалось установить соединение");

        // Ждем завершения аутентификации
        println!("⏳ Ждем завершения аутентификации...");
        auth_completion_task1.await
            .expect("❌ Задача завершения аутентификации для ноды 1 завершилась с ошибкой (join)")
            .expect("❌ Задача завершения аутентификации для ноды 1 завершилась с ошибкой (task)");
        auth_completion_task2.await
            .expect("❌ Задача завершения аутентификации для ноды 2 завершилась с ошибкой (join)")
            .expect("❌ Задача завершения аутентификации для ноды 2 завершилась с ошибкой (task)");
        auto_respond_task1.await
            .expect("❌ Задача автоматического ответа для ноды 1 завершилась с ошибкой (join)")
            .expect("❌ Задача автоматического ответа для ноды 1 завершилась с ошибкой (task)");
        auto_respond_task2.await
            .expect("❌ Задача автоматического ответа для ноды 2 завершилась с ошибкой (join)")
            .expect("❌ Задача автоматического ответа для ноды 2 завершилась с ошибкой (task)");

        println!("✅ Автоматическая аутентификация успешно завершена");

        // Проверяем финальное состояние
        let final_state = node1.commander.get_network_state().await
            .expect("❌ Не удалось получить финальное состояние");
        
        assert!(!final_state.connected_peers.is_empty(), 
            "❌ Нет подключенных пиров после аутентификации");
        
        let node2_in_peers = final_state.connected_peers.iter()
            .any(|peer| peer == node2.peer_id());
        
        assert!(node2_in_peers, "❌ Нода2 не найдена в списке подключенных пиров");

        println!("✅ Финальное состояние корректно:");
        println!("   Подключенные пиры: {}", final_state.connected_peers.len());
        println!("   Нода2 подключена: {}", node2_in_peers);

        // Graceful shutdown
        println!("🛑 Выполняем graceful shutdown...");
        node1.commander.shutdown().await.expect("❌ Не удалось остановить ноду 1");
        node2.commander.shutdown().await.expect("❌ Не удалось остановить ноду 2");
        node1.wait_for_shutdown().await.expect("❌ Ошибка при ожидании завершения ноды 1");
        node2.wait_for_shutdown().await.expect("❌ Ошибка при ожидании завершения ноды 2");

        println!("🎉 Тест автоматической аутентификации успешно завершен!");
    }).await;

    match result {
        Ok(_) => println!("✅ Тест выполнен за 10 секунд"),
        Err(_) => panic!("❌ ТЕСТ ПРЕВЫСИЛ ЛИМИТ В 10 СЕКУНД"),
    }
}

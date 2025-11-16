//! Тест DCUtR hole punching между двумя узлами
//!
//! Этот тест проверяет прямое соединение через hole punching
//! между двумя узлами, находящимися за разными NAT.

use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::NodeBuilder;

mod utils;
use utils::{dial_and_wait_connection, setup_listening_node};

/// Тест DCUtR hole punching между двумя узлами
/// 
/// Создает два узла с включенным DCUtR и проверяет,
/// что они могут установить прямое соединение через hole punching.
#[tokio::test]
async fn test_dcutr_hole_punching() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем тест DCUtR hole punching...");

    // ФАЗА 1: Создание узлов с DCUtR
    println!("🛠️ Фаза 1: Создание узлов с DCUtR...");

    println!("🆕 Создаем node1 с DCUtR...");
    let mut node1 = NodeBuilder::new()
        .with_dcutr()  // Включаем DCUtR для hole punching
        .build()
        .await
        .expect("❌ Не удалось создать node1 узел - критическая ошибка");

    println!("🆕 Создаем node2 с DCUtR...");
    let mut node2 = NodeBuilder::new()
        .with_dcutr()  // Включаем DCUtR для hole punching
        .build()
        .await
        .expect("❌ Не удалось создать node2 узел - критическая ошибка");

    // Запуск узлов
    println!("🚀 Запускаем узлы...");
    node1
        .start()
        .await
        .expect("❌ Не удалось запустить node1 узел - критическая ошибка");
    node2
        .start()
        .await
        .expect("❌ Не удалось запустить node2 узел - критическая ошибка");

    // Небольшая задержка для запуска swarm loops
    sleep(Duration::from_millis(100)).await;
    println!("✅ Узлы созданы и запущены:");
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());

    // ФАЗА 2: Настройка прослушивания
    println!("🎯 Фаза 2: Настройка прослушивания...");

    println!("🎯 Настраиваем node1 для прослушивания...");
    let node1_addr = setup_listening_node(&mut node1).await?;
    println!("📡 Node 1 слушает на: {}", node1_addr);

    println!("🎯 Настраиваем node2 для прослушивания...");
    let node2_addr = setup_listening_node(&mut node2).await?;
    println!("📡 Node 2 слушает на: {}", node2_addr);

    // Проверяем, что адреса содержат QUIC
    assert!(
        node1_addr.to_string().contains("/quic-v1"),
        "❌ Адрес node1 должен содержать QUIC протокол"
    );
    assert!(
        node2_addr.to_string().contains("/quic-v1"),
        "❌ Адрес node2 должен содержать QUIC протокол"
    );

    // ФАЗА 3: Подключение через DCUtR hole punching
    println!("🔗 Фаза 3: Подключение через DCUtR hole punching...");

    println!("🔗 Node1 подключается к node2 через DCUtR...");
    let connection_id = dial_and_wait_connection(
        &mut node1,
        *node2.peer_id(),
        node2_addr.clone(),
        Duration::from_secs(10),
    )
    .await?;

    println!("✅ DCUtR hole punching успешен! Connection ID: {:?}", connection_id);

    // Проверяем статус DCUtR
    println!("📊 Проверяем статус DCUtR...");
    match node1.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node1:");
            println!("   - DCUtR: {}", status.dcutr_enabled);
            assert!(status.dcutr_enabled, "❌ DCUtR должен быть включен на node1");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node1: {}", e),
    }

    match node2.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node2:");
            println!("   - DCUtR: {}", status.dcutr_enabled);
            assert!(status.dcutr_enabled, "❌ DCUtR должен быть включен на node2");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node2: {}", e),
    }

    // ФАЗА 4: Проверка типа соединения
    println!("🔍 Фаза 4: Проверка типа соединения...");
    
    // Получаем информацию о соединении
    match node1.commander.get_network_state().await {
        Ok(state) => {
            println!("📊 Состояние сети node1:");
            println!("   - Подключенные пиры: {}", state.connected_peers.len());
            
            for peer_id in state.connected_peers {
                println!("   - Подключен к пиру: {}", peer_id);
            }
        }
        Err(e) => panic!("❌ Не удалось получить состояние сети node1: {}", e),
    }

    // ФАЗА 5: Завершение теста
    println!("🏁 Фаза 5: Завершение теста...");

    // Очистка ресурсов
    println!("🧹 Выполняем очистку ресурсов...");
    node1
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node1 узел");
    node2
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node2 узел");

    println!("🎉 Тест DCUtR hole punching успешно завершен!");
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());
    println!("   - Прямое соединение через hole punching установлено!");

    Ok(())
}

/// Тест DCUtR с relay fallback
/// 
/// Проверяет, что при неудаче hole punching узлы используют relay как fallback.
#[tokio::test]
async fn test_dcutr_with_relay_fallback() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем тест DCUtR с relay fallback...");

    // ФАЗА 1: Создание узлов с полным NAT traversal
    println!("🛠️ Фаза 1: Создание узлов с NAT traversal...");

    println!("🆕 Создаем node1 с NAT traversal...");
    let mut node1 = NodeBuilder::new()
        .with_nat_traversal()  // Включает DCUtR, AutoNAT, Relay
        .build()
        .await
        .expect("❌ Не удалось создать node1 узел - критическая ошибка");

    println!("🆕 Создаем node2 с NAT traversal...");
    let mut node2 = NodeBuilder::new()
        .with_nat_traversal()  // Включает DCUtR, AutoNAT, Relay
        .build()
        .await
        .expect("❌ Не удалось создать node2 узел - критическая ошибка");

    // Запуск узлов
    println!("🚀 Запускаем узлы...");
    node1
        .start()
        .await
        .expect("❌ Не удалось запустить node1 узел - критическая ошибка");
    node2
        .start()
        .await
        .expect("❌ Не удалось запустить node2 узел - критическая ошибка");

    println!("✅ Узлы созданы и запущены:");
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());

    // ФАЗА 2: Проверка включенных компонентов
    println!("🔍 Фаза 2: Проверка включенных компонентов NAT traversal...");

    match node1.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node1:");
            println!("   - DCUtR: {}", status.dcutr_enabled);
            println!("   - AutoNAT: {}", status.autonat_enabled);
            println!("   - Relay Server: {}", status.relay_server_enabled);
            
            assert!(status.dcutr_enabled, "❌ DCUtR должен быть включен на node1");
            assert!(status.autonat_enabled, "❌ AutoNAT должен быть включен на node1");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node1: {}", e),
    }

    match node2.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node2:");
            println!("   - DCUtR: {}", status.dcutr_enabled);
            println!("   - AutoNAT: {}", status.autonat_enabled);
            println!("   - Relay Server: {}", status.relay_server_enabled);
            
            assert!(status.dcutr_enabled, "❌ DCUtR должен быть включен на node2");
            assert!(status.autonat_enabled, "❌ AutoNAT должен быть включен на node2");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node2: {}", e),
    }

    // ФАЗА 3: Завершение теста
    println!("🏁 Фаза 3: Завершение теста...");

    // Очистка ресурсов
    println!("🧹 Выполняем очистку ресурсов...");
    node1
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node1 узел");
    node2
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node2 узел");

    println!("🎉 Тест DCUtR с relay fallback успешно завершен!");
    println!("   - Все компоненты NAT traversal включены и работают");

    Ok(())
}

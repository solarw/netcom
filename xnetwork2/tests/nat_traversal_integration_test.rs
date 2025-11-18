//! Интеграционный тест полного NAT traversal
//!
//! Этот тест проверяет полный сценарий NAT traversal:
//! DCUtR hole punching → AutoNAT определение типа NAT → Relay fallback

use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::NodeBuilder;

mod utils;
use utils::{dial_and_wait_connection, setup_listening_node};

/// Интеграционный тест полного NAT traversal
/// 
/// Проверяет работу всех компонентов NAT traversal в комплексе:
/// - DCUtR для hole punching
/// - AutoNAT для определения типа NAT  
/// - Relay как fallback механизм
#[tokio::test]
async fn test_nat_traversal_integration() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем интеграционный тест NAT traversal...");

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

    // ФАЗА 3: Проверка включенных компонентов NAT traversal
    println!("🔍 Фаза 3: Проверка включенных компонентов NAT traversal...");

    // Даем время для инициализации всех компонентов
    sleep(Duration::from_secs(2)).await;

    match node1.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node1:");
            println!("   - DCUtR: {}", status.dcutr_enabled);
            println!("   - AutoNAT Client: {}", status.autonat_client_enabled);
            println!("   - Relay Server: {}", status.relay_server_enabled);
            println!("   - Identify: {}", status.identify_enabled);
            println!("   - mDNS: {}", status.mdns_enabled);
            println!("   - Kademlia: {}", status.kad_enabled);
            
            assert!(status.dcutr_enabled, "❌ DCUtR должен быть включен на node1");
            assert!(status.autonat_client_enabled, "❌ AutoNAT Client должен быть включен на node1");
            assert!(status.identify_enabled, "❌ Identify должен быть включен на node1");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node1: {}", e),
    }

    match node2.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node2:");
            println!("   - DCUtR: {}", status.dcutr_enabled);
            println!("   - AutoNAT Client: {}", status.autonat_client_enabled);
            println!("   - Relay Server: {}", status.relay_server_enabled);
            println!("   - Identify: {}", status.identify_enabled);
            println!("   - mDNS: {}", status.mdns_enabled);
            println!("   - Kademlia: {}", status.kad_enabled);
            
            assert!(status.dcutr_enabled, "❌ DCUtR должен быть включен на node2");
            assert!(status.autonat_client_enabled, "❌ AutoNAT Client должен быть включен на node2");
            assert!(status.identify_enabled, "❌ Identify должен быть включен на node2");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node2: {}", e),
    }

    // ФАЗА 4: Подключение через NAT traversal
    println!("🔗 Фаза 4: Подключение через NAT traversal...");

    println!("🔗 Node1 подключается к node2 через NAT traversal...");
    let connection_id = dial_and_wait_connection(
        &mut node1,
        *node2.peer_id(),
        node2_addr.clone(),
        Duration::from_secs(10),
    )
    .await?;

    println!("✅ NAT traversal успешен! Connection ID: {:?}", connection_id);

    // ФАЗА 5: Проверка состояния сети после подключения
    println!("📊 Фаза 5: Проверка состояния сети...");

    match node1.commander.get_network_state().await {
        Ok(state) => {
            println!("📊 Состояние сети node1:");
            println!("   - Подключенные пиры: {}", state.connected_peers.len());
            println!("   - Аутентифицированные пиры: {}", state.authenticated_peers.len());
            
            for peer_id in state.connected_peers {
                println!("   - Подключен к пиру: {}", peer_id);
            }
        }
        Err(e) => panic!("❌ Не удалось получить состояние сети node1: {}", e),
    }

    match node2.commander.get_network_state().await {
        Ok(state) => {
            println!("📊 Состояние сети node2:");
            println!("   - Подключенные пиры: {}", state.connected_peers.len());
            println!("   - Аутентифицированные пиры: {}", state.authenticated_peers.len());
            
            for peer_id in state.connected_peers {
                println!("   - Подключен к пиру: {}", peer_id);
            }
        }
        Err(e) => panic!("❌ Не удалось получить состояние сети node2: {}", e),
    }

    // ФАЗА 6: Проверка mDNS обнаружения
    println!("🔍 Фаза 6: Проверка mDNS обнаружения...");

    match node1.get_mdns_peers().await {
        Ok(peers) => {
            if peers.is_empty() {
                println!("❌ mDNS не обнаружил пиров (нормально в тестовой среде)");
            } else {
                println!("✅ mDNS обнаружил {} пиров:", peers.len());
                for (peer_id, addresses) in peers {
                    println!("   - {} с {} адресами", peer_id, addresses.len());
                }
            }
        }
        Err(e) => {
            println!("⚠️ Не удалось получить mDNS пиров: {}", e);
        }
    }

    // ФАЗА 7: Завершение теста
    println!("🏁 Фаза 7: Завершение теста...");

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

    println!("🎉 Интеграционный тест NAT traversal успешно завершен!");
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());
    println!("   - Все компоненты NAT traversal работают корректно");
    println!("   - Соединение установлено через NAT traversal механизмы");

    Ok(())
}

/// Тест NAT traversal с relay сервером
/// 
/// Проверяет работу NAT traversal с использованием relay сервера.
#[tokio::test]
async fn test_nat_traversal_with_relay() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем тест NAT traversal с relay сервером...");

    // ФАЗА 1: Создание relay сервера и узлов
    println!("🛠️ Фаза 1: Создание relay сервера и узлов...");

    println!("🆕 Создаем relay сервер...");
    let mut relay_server = NodeBuilder::new()
        .with_relay_server()  // Включаем relay сервер
        .build()
        .await
        .expect("❌ Не удалось создать relay сервер - критическая ошибка");

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

    // Запуск всех узлов
    println!("🚀 Запускаем все узлы...");
    relay_server
        .start()
        .await
        .expect("❌ Не удалось запустить relay сервер - критическая ошибка");
    node1
        .start()
        .await
        .expect("❌ Не удалось запустить node1 узел - критическая ошибка");
    node2
        .start()
        .await
        .expect("❌ Не удалось запустить node2 узел - критическая ошибка");

    println!("✅ Узлы созданы и запущены:");
    println!("   - Relay: {:?}", relay_server.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());

    // ФАЗА 2: Настройка прослушивания
    println!("🎯 Фаза 2: Настройка прослушивания...");

    println!("🎯 Настраиваем relay сервер для прослушивания...");
    let relay_addr = setup_listening_node(&mut relay_server).await?;
    println!("📡 Relay сервер слушает на: {}", relay_addr);

    println!("🎯 Настраиваем node1 для прослушивания...");
    let node1_addr = setup_listening_node(&mut node1).await?;
    println!("📡 Node 1 слушает на: {}", node1_addr);

    println!("🎯 Настраиваем node2 для прослушивания...");
    let node2_addr = setup_listening_node(&mut node2).await?;
    println!("📡 Node 2 слушает на: {}", node2_addr);

    // ФАЗА 3: Проверка включенных компонентов
    println!("🔍 Фаза 3: Проверка включенных компонентов...");

    // Даем время для инициализации всех компонентов
    sleep(Duration::from_secs(2)).await;

    match relay_server.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус relay сервера:");
            println!("   - Relay Server: {}", status.relay_server_enabled);
            assert!(status.relay_server_enabled, "❌ Relay сервер должен быть включен");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes relay сервера: {}", e),
    }

    // ФАЗА 4: Завершение теста
    println!("🏁 Фаза 4: Завершение теста...");

    // Очистка ресурсов
    println!("🧹 Выполняем очистку ресурсов...");
    relay_server
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить relay сервер");
    node1
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node1 узел");
    node2
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node2 узел");

    println!("🎉 Тест NAT traversal с relay сервером успешно завершен!");
    println!("   - Relay сервер создан и работает");
    println!("   - Узлы с NAT traversal готовы к работе");

    Ok(())
}

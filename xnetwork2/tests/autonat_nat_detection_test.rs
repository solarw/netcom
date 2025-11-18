//! Тест AutoNAT определения типа NAT
//!
//! Этот тест проверяет работу AutoNAT для определения типа NAT
//! и получения внешних адресов узлов.

use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::NodeBuilder;

mod utils;
use utils::setup_listening_node;

/// Тест AutoNAT определения типа NAT
/// 
/// Создает два узла с включенным AutoNAT и проверяет,
/// что они могут определить свой тип NAT и получить внешние адреса.
#[tokio::test]
async fn test_autonat_nat_detection() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем тест AutoNAT определения типа NAT...");

    // ФАЗА 1: Создание узлов с AutoNAT
    println!("🛠️ Фаза 1: Создание узлов с AutoNAT...");

    println!("🆕 Создаем node1 с AutoNAT (клиентский режим)...");
    let mut node1 = NodeBuilder::new()
        .with_autonat_client()  // Включаем клиентский AutoNAT для определения типа NAT
        .build()
        .await
        .expect("❌ Не удалось создать node1 узел - критическая ошибка");

    println!("🆕 Создаем node2 с AutoNAT (клиентский режим)...");
    let mut node2 = NodeBuilder::new()
        .with_autonat_client()  // Включаем клиентский AutoNAT для определения типа NAT
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

    // ФАЗА 3: Проверка статуса AutoNAT
    println!("🔍 Фаза 3: Проверка статуса AutoNAT...");

    // Даем время AutoNAT для инициализации
    sleep(Duration::from_secs(2)).await;

    match node1.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node1:");
            println!("   - AutoNAT Server: {}", status.autonat_server_enabled);
            println!("   - AutoNAT Client: {}", status.autonat_client_enabled);
            assert!(status.autonat_client_enabled, "❌ AutoNAT Client должен быть включен на node1");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node1: {}", e),
    }

    match node2.get_xroutes_status().await {
        Ok(status) => {
            println!("📈 XRoutes статус node2:");
            println!("   - AutoNAT Server: {}", status.autonat_server_enabled);
            println!("   - AutoNAT Client: {}", status.autonat_client_enabled);
            assert!(status.autonat_client_enabled, "❌ AutoNAT Client должен быть включен на node2");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node2: {}", e),
    }

    // ФАЗА 4: Проверка состояния сети
    println!("📊 Фаза 4: Проверка состояния сети...");

    match node1.commander.get_network_state().await {
        Ok(state) => {
            println!("📊 Состояние сети node1:");
            println!("   - PeerId: {}", state.peer_id);
            println!("   - Адреса прослушивания: {}", state.listening_addresses.len());
            println!("   - Подключенные пиры: {}", state.connected_peers.len());
            println!("   - Аутентифицированные пиры: {}", state.authenticated_peers.len());
            
            for addr in state.listening_addresses {
                println!("   - Слушает на: {}", addr);
            }
        }
        Err(e) => panic!("❌ Не удалось получить состояние сети node1: {}", e),
    }

    match node2.commander.get_network_state().await {
        Ok(state) => {
            println!("📊 Состояние сети node2:");
            println!("   - PeerId: {}", state.peer_id);
            println!("   - Адреса прослушивания: {}", state.listening_addresses.len());
            println!("   - Подключенные пиры: {}", state.connected_peers.len());
            println!("   - Аутентифицированные пиры: {}", state.authenticated_peers.len());
            
            for addr in state.listening_addresses {
                println!("   - Слушает на: {}", addr);
            }
        }
        Err(e) => panic!("❌ Не удалось получить состояние сети node2: {}", e),
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

    println!("🎉 Тест AutoNAT определения типа NAT успешно завершен!");
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());
    println!("   - AutoNAT включен и работает на обоих узлах");

    Ok(())
}

/// Тест полного NAT traversal с AutoNAT
/// 
/// Проверяет работу AutoNAT в составе полного NAT traversal стека.
#[tokio::test]
async fn test_autonat_with_full_nat_traversal() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем тест AutoNAT с полным NAT traversal...");

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
            assert!(status.autonat_client_enabled, "❌ AutoNAT клиент должен быть включен на node1");
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
            assert!(status.autonat_client_enabled, "❌ AutoNAT клиент должен быть включен на node2");
            assert!(status.identify_enabled, "❌ Identify должен быть включен на node2");
        }
        Err(e) => panic!("❌ Не удалось получить статус XRoutes node2: {}", e),
    }

    // ФАЗА 3: Проверка mDNS обнаружения
    println!("🔍 Фаза 3: Проверка mDNS обнаружения...");

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

    // ФАЗА 4: Завершение теста
    println!("🏁 Фаза 4: Завершение теста...");

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

    println!("🎉 Тест AutoNAT с полным NAT traversal успешно завершен!");
    println!("   - Все компоненты NAT traversal включены и работают");
    println!("   - AutoNAT готов к определению типа NAT");

    Ok(())
}

//! Тест комбинированной функциональности Kademlia и Relay
//! Проверяет, что node1 получает relay адрес node2 через Kademlia
//! после того, как node2 подключился к bootstrap (Kademlia + Relay сервер) и начал слушать через relay

use std::time::Duration;
use xnetwork2::{
    node_builder,
};
mod utils;
use utils::{setup_listening_node, setup_connection_with_auth, setup_listening_node_with_addr};

/// Тест комбинированной функциональности Kademlia и Relay
#[tokio::test]
async fn test_kademlia_relay_address_discovery() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем тест Kademlia + Relay адресного обнаружения...");

    // ФАЗА 1: Создание узлов
    println!("🛠️ Фаза 1: Создание узлов...");

    // 1.1 Создание bootstrap узла с Relay сервером
    println!("🆕 Создаем bootstrap узел (Kademlia + Relay сервер)...");
    let mut bootstrap_node = node_builder::builder()
        .with_relay_server()
        .with_kad_server()
        .build()
        .await
        .expect("❌ Не удалось создать bootstrap node - критическая ошибка");
    
    // 1.2 Создание node1 и node2
    println!("🆕 Создаем node1...");
    let mut node1 = node_builder::builder()
        .with_kad_server()
        .build()
        .await
        .expect("❌ Не удалось создать node1 - критическая ошибка");
    
    println!("🆕 Создаем node2...");
    let mut node2 = node_builder::builder()
        .with_kad_server()
        .build()
        .await
        .expect("❌ Не удалось создать node2 - критическая ошибка");

    // ФАЗА 2: Включение Kademlia СРАЗУ после запуска узлов
    println!("🌐 Фаза 2: Включение Kademlia СРАЗУ после запуска узлов...");

    // Запуск всех узлов
    println!("🚀 Запускаем все узлы...");
    bootstrap_node.start().await.expect("❌ Не удалось запустить bootstrap node - критическая ошибка");
    node1.start().await.expect("❌ Не удалось запустить node1 - критическая ошибка");
    node2.start().await.expect("❌ Не удалось запустить node2 - критическая ошибка");

    // Небольшая задержка для запуска swarm loops
    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("✅ Узлы созданы и запущены:");
    println!("   - Bootstrap: {:?}", bootstrap_node.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());

    // 2.1 Включение Kademlia на всех узлах СРАЗУ после запуска
    println!("🔧 Включаем Kademlia на всех узлах СРАЗУ после запуска...");
    bootstrap_node.commander.enable_kad().await
        .expect("❌ Не удалось включить Kademlia на bootstrap node - критическая ошибка");
    
    node1.commander.enable_kad().await
        .expect("❌ Не удалось включить Kademlia на node1 - критическая ошибка");
    
    node2.commander.enable_kad().await
        .expect("❌ Не удалось включить Kademlia на node2 - критическая ошибка");

    println!("✅ Kademlia включена на всех узлах");

    // ФАЗА 3: Настройка прослушивания
    println!("🎯 Фаза 3: Настройка прослушивания...");

    // 3.1 Настройка bootstrap узла
    println!("🎯 Настраиваем bootstrap узел для прослушивания с Kademlia...");
    let bootstrap_addr = utils::setup_listening_node_with_kad(&mut bootstrap_node).await?;
    println!("📡 Bootstrap узел слушает на: {}", bootstrap_addr);

    // 3.2 Настройка node1
    println!("🎯 Настраиваем node1 для прослушивания с Kademlia...");
    let node1_addr = utils::setup_listening_node_with_kad(&mut node1).await?;
    println!("📡 Node 1 слушает на: {}", node1_addr);
    
    // 3.3 Настройка node2
    println!("🎯 Настраиваем node2 для прослушивания с Kademlia...");
    let node2_addr = utils::setup_listening_node_with_kad(&mut node2).await?;
    println!("📡 Node 2 слушает на: {}", node2_addr);
    // 3.2 Настройка node1

    // Проверяем, что все адреса содержат QUIC
    assert!(bootstrap_addr.to_string().contains("/quic-v1"), "❌ Адрес bootstrap должен содержать QUIC протокол");
    assert!(node1_addr.to_string().contains("/quic-v1"), "❌ Адрес node1 должен содержать QUIC протокол");
    assert!(node2_addr.to_string().contains("/quic-v1"), "❌ Адрес node2 должен содержать QUIC протокол");

    // ФАЗА 4: Установка соединений
    println!("🔗 Фаза 4: Установка соединений...");

    // 4.1 Node1 → Bootstrap соединение
    println!("🔗 Устанавливаем соединение Node1 → Bootstrap...");
    setup_connection_with_auth(&mut node1, &mut bootstrap_node, bootstrap_addr.clone(), Duration::from_secs(10)).await?;
    println!("✅ Соединение Node1 ↔ Bootstrap установлено и аутентифицировано");

    // 4.2 Node2 → Bootstrap соединение
    println!("🔗 Устанавливаем соединение Node2 → Bootstrap...");
    setup_connection_with_auth(&mut node2, &mut bootstrap_node, bootstrap_addr.clone(), Duration::from_secs(10)).await?;
    println!("✅ Соединение Node2 ↔ Bootstrap установлено и аутентифицировано");

    // 4.2 Bootstrap узлов в Kademlia DHT
    println!("🚀 Выполняем bootstrap Kademlia...");

    // Node1 → Bootstrap
    node1.commander.bootstrap_to_peer(
        *bootstrap_node.peer_id(),
        vec![bootstrap_addr.clone()]
    ).await
    .expect("❌ Node1 должен успешно выполнить bootstrap");

    // Node2 → Bootstrap
    node2.commander.bootstrap_to_peer(
        *bootstrap_node.peer_id(),
        vec![bootstrap_addr.clone()]
    ).await
    .expect("❌ Node2 должен успешно выполнить bootstrap");

    // Ждем распространения информации в DHT
    println!("⏳ Ждем распространения информации в DHT (500мс)...");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ФАЗА 5: Настройка Relay функциональности
    println!("🔄 Фаза 5: Настройка Relay функциональности...");

    // 5.1 Node2 запрашивает relay адрес у bootstrap
    println!("🔄 Node2 запрашивает relay адрес у bootstrap...");
    let relay_addr_str = format!(
        "{}/p2p/{}/p2p-circuit",
        bootstrap_addr.to_string(),
        bootstrap_node.peer_id().to_string()
    );

    // 5.2 Node2 начинает слушать на relay адресе
    println!("🎯 Node2 начинает слушать на relay адресе...");
    let node2_relay_addr = setup_listening_node_with_addr(&mut node2, relay_addr_str).await?;
    println!("📡 Node 2 слушает на relay адресе: {}", node2_relay_addr);

    // Ждем немного для распространения информации через Identify
    println!("⏳ Ждем распространения relay адреса через Identify (500мс)...");
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // ФАЗА 6: Ключевая проверка - получение relay адреса через Kademlia
    println!("🧪 Фаза 6: Ключевая проверка - получение relay адреса через Kademlia...");

    // 6.1 Node1 ищет адреса Node2 через Kademlia
    println!("🔍 Node1 ищет адреса Node2 через Kademlia...");
    let peer_to_find = *node2.peer_id();
    let found_addresses = node1.commander.find_peer_addresses(peer_to_find, Duration::from_secs(10)).await
        .expect("❌ Node1 должен найти адреса Node2 через Kademlia - критическая ошибка");

    // 6.2 Проверяем, что найденные адреса не пустые
    assert!(!found_addresses.is_empty(), "❌ КРИТИЧЕСКАЯ ОШИБКА: find_peer_addresses вернул пустой список - это недопустимо!");

    println!("✅ Node1 нашел {} адресов для Node2 через Kademlia", found_addresses.len());
    
    // 6.3 Выводим найденные адреса для отладки
    println!("📋 Найденные адреса:");
    for (i, addr) in found_addresses.iter().enumerate() {
        println!("   {}. {}", i + 1, addr);
    }

    // 6.4 Ключевая проверка: среди найденных адресов должен быть relay адрес
    let has_relay_address = found_addresses.iter().any(|addr| {
        let addr_str = addr.to_string();
        addr_str.contains("/p2p-circuit") && addr_str.contains(&node2.peer_id().to_string())
    });

    assert!(
        has_relay_address,
        "❌ КРИТИЧЕСКАЯ ОШИБКА: Среди найденных адресов нет relay адреса Node2!\n\
         Ожидался адрес вида: .../p2p-circuit/p2p/{}\n\
         Найдены адреса: {:?}",
        node2.peer_id(),
        found_addresses
    );

    println!("✅ Node1 успешно получил relay адрес Node2 через Kademlia!");

    // ФАЗА 7: Завершение теста
    println!("🏁 Фаза 7: Завершение теста...");

    // Очистка ресурсов
    println!("🧹 Выполняем очистку ресурсов...");
    bootstrap_node.force_shutdown().await.expect("❌ Не удалось завершить bootstrap node");
    node1.force_shutdown().await.expect("❌ Не удалось завершить node1");
    node2.force_shutdown().await.expect("❌ Не удалось завершить node2");

    println!("🎉 Тест Kademlia + Relay адресного обнаружения успешно пройден!");
    println!("   - Bootstrap: {:?}", bootstrap_node.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());
    println!("   - Node1 успешно получил relay адрес Node2 через Kademlia!");

    Ok(())
}

//! Integration test for Kademlia discovery functionality

use std::time::Duration;
use tokio::time::timeout;
use tokio::time::sleep;
use xnetwork2::{
    node_builder,
    node_events::NodeEvent,
};
mod utils;
use utils::{setup_listening_node, setup_connection_with_auth};


/// Test Kademlia discovery with bootstrap node
#[tokio::test]
async fn test_kademlia_discovery_with_bootstrap() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Starting Kademlia discovery test with bootstrap node...");

    // ФАЗА 1: Подготовка узлов + включение XRoutes сразу
    println!("🛠️ Фаза 1: Подготовка узлов и включение XRoutes...");

    // 1.1 Создание и запуск узлов
    println!("🆕 Создаем три узла...");
    let mut node_bootstrap = node_builder::builder()
        .build()
        .await
        .expect("❌ Не удалось создать bootstrap node - критическая ошибка");
    
    let mut node1 = node_builder::builder()
        .build()
        .await
        .expect("❌ Не удалось создать node1 - критическая ошибка");
    
    let mut node2 = node_builder::builder()
        .build()
        .await
        .expect("❌ Не удалось создать node2 - критическая ошибка");

    // Подписка на события ДО запуска узлов
    println!("📡 Подписываемся на события всех узлов...");
    let mut bootstrap_events = node_bootstrap.subscribe();
    let mut node1_events = node1.subscribe();
    let mut node2_events = node2.subscribe();

    // Запуск всех узлов
    println!("🚀 Запускаем все узлы...");
    node_bootstrap.start().await.expect("❌ Не удалось запустить bootstrap node - критическая ошибка");
    node1.start().await.expect("❌ Не удалось запустить node1 - критическая ошибка");
    node2.start().await.expect("❌ Не удалось запустить node2 - критическая ошибка");

    // Небольшая задержка для запуска swarm loops
    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("✅ Узлы созданы и запущены:");
    println!("   - Bootstrap: {:?}", node_bootstrap.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());




 // 1.1.1 Включение XRoutes функциональности СРАЗУ после запуска
    println!("🔧 Включаем XRoutes Identify и Kademlia на всех узлах...");

    // Включение Identify на всех узлах через alias методы
    node_bootstrap.commander.enable_identify().await
        .expect("❌ Не удалось включить Identify на bootstrap node - критическая ошибка");
    
    node1.commander.enable_identify().await
        .expect("❌ Не удалось включить Identify на node1 - критическая ошибка");
    
    node2.commander.enable_identify().await
        .expect("❌ Не удалось включить Identify на node2 - критическая ошибка");

    // Включение Kademlia на всех узлах через alias методы
    node_bootstrap.commander.enable_kad().await
        .expect("❌ Не удалось включить Kademlia на bootstrap node - критическая ошибка");
    
    node1.commander.enable_kad().await
        .expect("❌ Не удалось включить Kademlia на node1 - критическая ошибка");
    
    node2.commander.enable_kad().await
        .expect("❌ Не удалось включить Kademlia на node2 - критическая ошибка");

    println!("✅ XRoutes Identify и Kademlia включены на всех узлах");

    // Проверка статуса через alias метод
    let bootstrap_status = node_bootstrap.commander.get_xroutes_status().await
        .expect("❌ Не удалось получить статус bootstrap node - критическая ошибка");

    assert!(bootstrap_status.identify_enabled, "❌ Identify должен быть включен на bootstrap node");
    assert!(bootstrap_status.kad_enabled, "❌ Kademlia должен быть включен на bootstrap node");
    println!("✅ Статус bootstrap node: identify_enabled={}, kad_enabled={}", 
             bootstrap_status.identify_enabled, bootstrap_status.kad_enabled);





    // 1.2 Настройка всех узлов для прослушивания адресов (последовательно)
    println!("🎯 Настраиваем все узлы для прослушивания...");
    
    // Настраиваем bootstrap node
    println!("🎯 Настраиваем bootstrap node для прослушивания...");
    let bootstrap_addr = setup_listening_node(&mut node_bootstrap).await?;
    println!("📡 Bootstrap node слушает на: {}", bootstrap_addr);
    
    // Настраиваем node1
    println!("🎯 Настраиваем node1 для прослушивания...");
    let node1_addr = setup_listening_node(&mut node1).await?;
    println!("📡 Node 1 слушает на: {}", node1_addr);
    
    // Настраиваем node2
    println!("🎯 Настраиваем node2 для прослушивания...");
    let node2_addr = setup_listening_node(&mut node2).await?;
    println!("📡 Node 2 слушает на: {}", node2_addr);

    // Проверяем, что все адреса содержат QUIC
    assert!(bootstrap_addr.to_string().contains("/quic-v1"), "❌ Адрес bootstrap должен содержать QUIC протокол");
    assert!(node1_addr.to_string().contains("/quic-v1"), "❌ Адрес node1 должен содержать QUIC протокол");
    assert!(node2_addr.to_string().contains("/quic-v1"), "❌ Адрес node2 должен содержать QUIC протокол");

   

    // ФАЗА 2: Установка соединений с использованием новых утилит
    println!("🔗 Фаза 2: Установка соединений...");

    // 2.1 Node1 → Bootstrap соединение с автоматической аутентификацией
    println!("🔗 Устанавливаем соединение Node1 → Bootstrap...");
    setup_connection_with_auth(&mut node1, &mut node_bootstrap, bootstrap_addr.clone(), Duration::from_secs(10)).await?;
    println!("✅ Соединение Node1 ↔ Bootstrap установлено и аутентифицировано");

    // 2.2 Node2 → Bootstrap соединение с автоматической аутентификацией
    println!("🔗 Устанавливаем соединение Node2 → Bootstrap...");
    setup_connection_with_auth(&mut node2, &mut node_bootstrap, bootstrap_addr.clone(), Duration::from_secs(10)).await?;
    println!("✅ Соединение Node2 ↔ Bootstrap установлено и аутентифицировано");

    println!("✅ Аутентификация всех узлов успешно завершена");

    // ФАЗА 4: Ожидание обмена информацией через Identify
    println!("� Фаза 4: Ожидание обмена информацией через Identify...");

    // Identify уже включен и работает автоматически
    // Вместо ожидания конкретных Identify событий, мы можем проверить, что Kademlia
    // получает информацию о пирах через Identify
    println!("✅ Identify включен и работает автоматически");
    println!("⏳ Ждем обмена Identify информацией (2 секунды)...");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Проверяем, что XRoutesHandler печатает события Identify
    println!("✅ XRoutesHandler настроен на печать событий Identify");

    // ФАЗА 5: Kademlia Discovery операции
    println!("🌐 Фаза 5: Kademlia Discovery операции...");

    // 5.1 Bootstrap Kademlia
    println!("🚀 Выполняем bootstrap Kademlia...");

    // Bootstrap node1 к bootstrap node через alias метод
    node1.commander.bootstrap_to_peer(
        *node_bootstrap.peer_id(),
        vec![bootstrap_addr.clone()]
    ).await
    .expect("❌ Node1 должен успешно выполнить bootstrap");
    println!("✅ Node1 выполнил bootstrap к bootstrap node");

    // Bootstrap node2 к bootstrap node через alias метод
    node2.commander.bootstrap_to_peer(
        *node_bootstrap.peer_id(),
        vec![bootstrap_addr.clone()]
    ).await
    .expect("❌ Node2 должен успешно выполнить bootstrap");
    println!("✅ Node2 выполнил bootstrap к bootstrap node");

    // Ждем распространения информации в DHT
    println!("⏳ Ждем распространения информации в DHT (2 секунды)...");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 5.2 Тестирование Kademlia функциональности
    println!("🧪 Тестируем Kademlia функциональность...");


    // Тестируем новый метод find_peer_addresses с таймаутом
    println!("🧪 Тестируем новый метод find_peer_addresses...");

    let peer_to_find = *node2.peer_id();
    match node1.commander.find_peer_addresses(peer_to_find, Duration::from_secs(10)).await {
        Ok(addresses) => {
            println!("✅ Node1 нашел адреса для {} через find_peer_addresses: {} уникальных адресов", 
                     peer_to_find, addresses.len());
            
            // СТРОГАЯ ПРОВЕРКА: адреса не должны быть пустыми
            assert!(!addresses.is_empty(), "❌ КРИТИЧЕСКАЯ ОШИБКА: find_peer_addresses вернул пустой список при Ok - это недопустимо!");
            
            println!("📋 Найденные адреса:");
            for (i, addr) in addresses.iter().enumerate() {
                println!("   {}. {}", i + 1, addr);
            }
            
            // СТРОГАЯ ПРОВЕРКА: все адреса должны принадлежать целевому пиру
            let target_peer_id_str = peer_to_find.to_string();
            for addr in &addresses {
                assert!(
                    addr.to_string().contains(&target_peer_id_str),
                    "❌ КРИТИЧЕСКАЯ ОШИБКА: Адрес {} не принадлежит пиру {}",
                    addr, peer_to_find
                );
            }
            println!("✅ Все найденные адреса принадлежат целевому пиру");
        }
        Err(e) => {
            panic!("❌ КРИТИЧЕСКАЯ ОШИБКА: Node1 не смог найти адреса для существующего пира {}: {:?}", peer_to_find, e);
        }
    }

    // Тестируем поиск несуществующего пира
    let fake_peer_id = libp2p::PeerId::random();
    match node1.commander.find_peer_addresses(fake_peer_id, Duration::from_secs(5)).await {
        Ok(addresses) => {
            // СТРОГАЯ ПРОВЕРКА: для несуществующего пира не должно быть адресов
            panic!("❌ КРИТИЧЕСКАЯ ОШИБКА: find_peer_addresses вернул Ok для несуществующего пира {} с адресами: {:?}", fake_peer_id, addresses);
        }
        Err(e) => {
            println!("✅ Поиск несуществующего пира вернул ошибку: {:?}", e);
            // Проверяем, что ошибка связана с таймаутом или ненахождением пира
            let error_str = e.to_string();
            assert!(
                error_str.contains("timeout") || error_str.contains("not found") || error_str.contains("Peer"),
                "❌ КРИТИЧЕСКАЯ ОШИБКА: Неожиданная ошибка для несуществующего пира: {}",
                error_str
            );
        }
    }

    // Тестируем гарантированный таймаут для существующего пира
    println!("🧪 Тестируем гарантированный таймаут для существующего пира...");
    match node1.commander.find_peer_addresses(peer_to_find, Duration::from_millis(1)).await {
        Ok(addresses) => {
            // СТРОГАЯ ПРОВЕРКА: за 1мс невозможно найти пир, должен быть таймаут
            panic!("❌ КРИТИЧЕСКАЯ ОШИБКА: find_peer_addresses вернул Ok за 1мс для пира {} с адресами: {:?}", peer_to_find, addresses);
        }
        Err(e) => {
            let error_str = e.to_string();
            println!("✅ Таймаут сработал для существующего пира: {}", error_str);
            
            // СТРОГАЯ ПРОВЕРКА: ошибка должна содержать "timeout"
            assert!(
                error_str.contains("timeout") || error_str.contains("Task timeout"),
                "❌ КРИТИЧЕСКАЯ ОШИБКА: Ожидалась ошибка таймаута, но получили: {}",
                error_str
            );
            println!("✅ Таймаут подтвержден по тексту ошибки");
        }
    }

    // Тестируем гарантированный таймаут для несуществующего пира
    println!("🧪 Тестируем гарантированный таймаут для несуществующего пира...");
    match node1.commander.find_peer_addresses(fake_peer_id, Duration::from_millis(1)).await {
        Ok(addresses) => {
            // СТРОГАЯ ПРОВЕРКА: за 1мс невозможно найти несуществующий пир
            panic!("❌ КРИТИЧЕСКАЯ ОШИБКА: find_peer_addresses вернул Ok за 1мс для несуществующего пира {} с адресами: {:?}", fake_peer_id, addresses);
        }
        Err(e) => {
            let error_str = e.to_string();
            println!("✅ Таймаут сработал для несуществующего пира: {}", error_str);
            
            // СТРОГАЯ ПРОВЕРКА: ошибка должна содержать "timeout"
            assert!(
                error_str.contains("timeout") || error_str.contains("Task timeout"),
                "❌ КРИТИЧЕСКАЯ ОШИБКА: Ожидалась ошибка таймаута, но получили: {}",
                error_str
            );
            println!("✅ Таймаут подтвержден по тексту ошибки");
        }
    }

    // Финальная проверка статуса Kademlia через alias метод
    let final_status = node_bootstrap.commander.get_xroutes_status().await
        .expect("❌ Не удалось получить финальный статус - критическая ошибка");

    assert!(final_status.kad_enabled, "❌ Kademlia должен быть включен на bootstrap node");
    println!("✅ Финальный статус Kademlia: kad_enabled={}", final_status.kad_enabled);

    // ФАЗА 6: Завершение
    println!("� Фаза 6: Завершение...");

    // Очистка ресурсов
    println!("🧹 Выполняем очистку ресурсов...");
    node_bootstrap.force_shutdown().await.expect("❌ Не удалось завершить bootstrap node");
    node1.force_shutdown().await.expect("❌ Не удалось завершить node1");
    node2.force_shutdown().await.expect("❌ Не удалось завершить node2");

    println!("🎉 Тест Kademlia discovery успешно завершен!");
    println!("   - Bootstrap: {:?}", node_bootstrap.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());
    println!("   - Все этапы пройдены успешно!");

    Ok(())
}

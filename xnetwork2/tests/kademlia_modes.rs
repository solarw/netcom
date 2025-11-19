//! Интеграционные тесты для проверки работы Kademlia серверного и клиентского режимов

use std::time::Duration;
use tokio::time::timeout;
use xnetwork2::node_builder::NodeBuilder;

/// Тест создания узла в серверном режиме Kademlia
#[tokio::test]
async fn test_kad_server_mode() {
    println!("🧪 Testing Kademlia SERVER mode...");
    
    // Создаем узел в серверном режиме
    let mut node = NodeBuilder::new()
        .with_kad_server()
        .build()
        .await
        .expect("Failed to create node with Kademlia server mode");
    
    // Проверяем, что узел создан успешно
    assert!(!node.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    // Запускаем узел
    let _handle = node.start().await.expect("Failed to start node");
    
    // Даем узлу время на инициализацию
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Останавливаем узел
    node.stop().await.expect("Failed to stop node");
    
    println!("✅ Kademlia SERVER mode test passed");
}

/// Тест создания узла в клиентском режиме Kademlia
#[tokio::test]
async fn test_kad_client_mode() {
    println!("🧪 Testing Kademlia CLIENT mode...");
    
    // Создаем узел в клиентском режиме
    let mut node = NodeBuilder::new()
        .with_kad_client()
        .build()
        .await
        .expect("Failed to create node with Kademlia client mode");
    
    // Проверяем, что узел создан успешно
    assert!(!node.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    // Запускаем узел
    let _handle = node.start().await.expect("Failed to start node");
    
    // Даем узлу время на инициализацию
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Останавливаем узел
    node.stop().await.expect("Failed to stop node");
    
    println!("✅ Kademlia CLIENT mode test passed");
}

/// Тест создания узла с legacy Kademlia (без указания режима)
#[tokio::test]
async fn test_kad_legacy_mode() {
    println!("🧪 Testing Kademlia LEGACY mode...");
    
    // Создаем узел с legacy Kademlia
    let mut node = NodeBuilder::new()
        .with_kademlia()
        .build()
        .await
        .expect("Failed to create node with legacy Kademlia");
    
    // Проверяем, что узел создан успешно
    assert!(!node.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    // Запускаем узел
    let _handle = node.start().await.expect("Failed to start node");
    
    // Даем узлу время на инициализацию
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Останавливаем узел
    node.stop().await.expect("Failed to stop node");
    
    println!("✅ Kademlia LEGACY mode test passed");
}

/// Тест проверки приоритетов режимов Kademlia
#[tokio::test]
async fn test_kad_mode_priority() {
    println!("🧪 Testing Kademlia mode priorities...");
    
    // Тест 1: Серверный режим должен иметь приоритет над клиентским
    let mut node1 = NodeBuilder::new()
        .with_kad_server()
        .with_kad_client() // Этот вызов должен быть проигнорирован
        .build()
        .await
        .expect("Failed to create node with Kademlia server priority");
    
    // Проверяем, что узел создан успешно
    assert!(!node1.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    // Запускаем и останавливаем узел
    let _handle1 = node1.start().await.expect("Failed to start node");
    tokio::time::sleep(Duration::from_secs(1)).await;
    node1.stop().await.expect("Failed to stop node");
    
    // Тест 2: Клиентский режим должен работать, когда серверный не указан
    let mut node2 = NodeBuilder::new()
        .with_kad_client()
        .build()
        .await
        .expect("Failed to create node with Kademlia client mode");
    
    // Проверяем, что узел создан успешно
    assert!(!node2.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    // Запускаем и останавливаем узел
    let _handle2 = node2.start().await.expect("Failed to start node");
    tokio::time::sleep(Duration::from_secs(1)).await;
    node2.stop().await.expect("Failed to stop node");
    
    println!("✅ Kademlia mode priorities test passed");
}

/// Тест с таймаутом для избежания зависания
#[tokio::test]
async fn test_kad_modes_with_timeout() {
    println!("🧪 Testing Kademlia modes with timeout...");
    
    // Тестируем серверный режим с таймаутом
    let result = timeout(Duration::from_secs(10), async {
        let mut node = NodeBuilder::new()
            .with_kad_server()
            .build()
            .await
            .expect("Failed to create node with Kademlia server mode");
        
        let _handle = node.start().await.expect("Failed to start node");
        tokio::time::sleep(Duration::from_secs(2)).await;
        node.stop().await.expect("Failed to stop node");
    }).await;
    
    assert!(result.is_ok(), "Kademlia server mode test should complete within timeout");
    
    // Тестируем клиентский режим с таймаутом
    let result = timeout(Duration::from_secs(10), async {
        let mut node = NodeBuilder::new()
            .with_kad_client()
            .build()
            .await
            .expect("Failed to create node with Kademlia client mode");
        
        let _handle = node.start().await.expect("Failed to start node");
        tokio::time::sleep(Duration::from_secs(2)).await;
        node.stop().await.expect("Failed to stop node");
    }).await;
    
    assert!(result.is_ok(), "Kademlia client mode test should complete within timeout");
    
    println!("✅ Kademlia modes with timeout test passed");
}

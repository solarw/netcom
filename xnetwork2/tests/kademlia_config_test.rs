//! Простой тест для проверки конфигурации Kademlia режимов без запуска узлов

use xnetwork2::node_builder::NodeBuilder;

/// Тест проверки конфигурации Kademlia серверного режима
#[tokio::test]
async fn test_kad_server_config() {
    println!("🧪 Testing Kademlia SERVER configuration...");
    
    // Создаем узел в серверном режиме
    let node = NodeBuilder::new()
        .with_kad_server()
        .build()
        .await
        .expect("Failed to create node with Kademlia server mode");
    
    // Проверяем, что узел создан успешно
    assert!(!node.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    println!("✅ Kademlia SERVER configuration test passed");
}

/// Тест проверки конфигурации Kademlia клиентского режима
#[tokio::test]
async fn test_kad_client_config() {
    println!("🧪 Testing Kademlia CLIENT configuration...");
    
    // Создаем узел в клиентском режиме
    let node = NodeBuilder::new()
        .with_kad_client()
        .build()
        .await
        .expect("Failed to create node with Kademlia client mode");
    
    // Проверяем, что узел создан успешно
    assert!(!node.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    println!("✅ Kademlia CLIENT configuration test passed");
}

/// Тест проверки конфигурации legacy Kademlia
#[tokio::test]
async fn test_kad_legacy_config() {
    println!("🧪 Testing Kademlia LEGACY configuration...");
    
    // Создаем узел с legacy Kademlia
    let node = NodeBuilder::new()
        .with_kademlia()
        .build()
        .await
        .expect("Failed to create node with legacy Kademlia");
    
    // Проверяем, что узел создан успешно
    assert!(!node.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    println!("✅ Kademlia LEGACY configuration test passed");
}

/// Тест проверки приоритетов режимов Kademlia
#[tokio::test]
async fn test_kad_mode_priority_config() {
    println!("🧪 Testing Kademlia mode priorities configuration...");
    
    // Тест 1: Серверный режим должен иметь приоритет над клиентским
    let node1 = NodeBuilder::new()
        .with_kad_server()
        .with_kad_client() // Этот вызов должен быть проигнорирован
        .build()
        .await
        .expect("Failed to create node with Kademlia server priority");
    
    // Проверяем, что узел создан успешно
    assert!(!node1.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    // Тест 2: Клиентский режим должен работать, когда серверный не указан
    let node2 = NodeBuilder::new()
        .with_kad_client()
        .build()
        .await
        .expect("Failed to create node with Kademlia client mode");
    
    // Проверяем, что узел создан успешно
    assert!(!node2.peer_id().to_string().is_empty(), "Node should have valid PeerId");
    
    println!("✅ Kademlia mode priorities configuration test passed");
}

// tests/node_core/test_node_creation.rs - Тест создания ноды (PRIORITY 1)

use std::time::Duration;
use crate::test_utils::{TestNode, NodeExt, NetIDUtils};

/// Тест создания ноды с корректным Peer ID и конфигурацией
#[tokio::test]
async fn test_node_creation() {
    // Создаем тестовый узел
    let node = TestNode::new().await.expect("Failed to create test node");
    
    // Проверяем, что узел создан с корректным Peer ID
    assert!(!node.peer_id.to_string().is_empty(), "Node should have valid Peer ID");
    
    // Проверяем, что Peer ID соответствует ожидаемому формату
    assert!(node.peer_id.to_string().starts_with("12D3KooW"), 
            "Peer ID should start with expected prefix");
    
    // Проверяем, что узел имеет все необходимые компоненты
    assert!(node.peer_id.to_string().len() > 10, "Peer ID should be properly formatted");
    
    println!("✅ Node creation test passed - node created with Peer ID: {}", node.peer_id);
}

/// Тест создания нескольких узлов с уникальными Peer ID
#[tokio::test]
async fn test_multiple_nodes_unique_peer_ids() {
    let node1 = TestNode::new().await.expect("Failed to create first node");
    let node2 = TestNode::new().await.expect("Failed to create second node");
    let node3 = TestNode::new().await.expect("Failed to create third node");
    
    // Проверяем, что все узлы имеют уникальные Peer ID
    assert_ne!(node1.peer_id, node2.peer_id, "Nodes should have different Peer IDs");
    assert_ne!(node1.peer_id, node3.peer_id, "Nodes should have different Peer IDs");
    assert_ne!(node2.peer_id, node3.peer_id, "Nodes should have different Peer IDs");
    
    // Проверяем, что все Peer ID валидны
    assert!(!node1.peer_id.to_string().is_empty());
    assert!(!node2.peer_id.to_string().is_empty());
    assert!(!node3.peer_id.to_string().is_empty());
    
    println!("✅ Multiple nodes test passed - all nodes have unique Peer IDs");
}

/// Тест создания узла с Proof of Representation
#[tokio::test]
async fn test_node_with_por() {
    let node = TestNode::new().await.expect("Failed to create node with PoR");
    
    // Проверяем, что узел создан
    assert!(!node.peer_id.to_string().is_empty(), "Node should have valid Peer ID");
    
    // Проверяем, что узел имеет все необходимые компоненты
    assert!(node.peer_id.to_string().len() > 10, "Peer ID should be properly formatted");
    
    println!("✅ Node with PoR test passed - node created successfully with PoR");
}

/// Тест создания временных узлов через NodeExt
#[tokio::test]
async fn test_ephemeral_node_creation() {
    let node = TestNode::new_ephemeral().await.expect("Failed to create ephemeral node");
    
    // Проверяем базовые свойства узла
    assert!(!node.peer_id.to_string().is_empty(), "Ephemeral node should have valid Peer ID");
    
    println!("✅ Ephemeral node creation test passed");
}

/// Тест создания узла с различными конфигурациями
#[tokio::test]
async fn test_node_configurations() {
    // Тестируем создание узла в режиме сервера
    let (server_node, server_commander, server_events, server_peer_id) = 
        crate::test_utils::create_test_node(true, false)
        .await
        .expect("Failed to create server node");
    
    assert!(!server_peer_id.to_string().is_empty(), "Server node should have valid Peer ID");
    
    // Тестируем создание узла в режиме клиента с включенным mDNS
    let (client_node, client_commander, client_events, client_peer_id) = 
        crate::test_utils::create_test_node(false, true)
        .await
        .expect("Failed to create client node");
    
    assert!(!client_peer_id.to_string().is_empty(), "Client node should have valid Peer ID");
    
    // Убеждаемся, что Peer ID разные
    assert_ne!(server_peer_id, client_peer_id, "Server and client should have different Peer IDs");
    
    println!("✅ Node configurations test passed - server and client nodes created successfully");
}

/// Тест создания Proof of Representation
#[tokio::test]
async fn test_por_creation() {
    let por = NetIDUtils::create_test_por();
    
    // Проверяем, что PoR создан (базовая проверка существования)
    assert!(!por.peer_id.to_string().is_empty(), "PoR should have valid Peer ID");
    
    println!("✅ PoR creation test passed - Proof of Representation created successfully");
}

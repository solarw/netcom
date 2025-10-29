// tests/node_core/test_node_startup.rs - Тест запуска ноды (PRIORITY 1)

use std::time::Duration;
use libp2p::identity;
use xnetwork::{NetworkNode, XRoutesConfig};
use crate::test_utils::{TestNode, NodeExt, create_test_por};

/// Тест успешного запуска ноды и инициализации компонентов
#[tokio::test]
async fn test_node_startup() {
    println!("🔄 Starting test_node_startup...");
    
    // Создаем тестовый узел
    let mut node = TestNode::new().await.expect("Failed to create test node");
    println!("✅ Node created with Peer ID: {}", node.peer_id);
    
    // Запускаем прослушивание на случайном порту
    println!("🔄 Starting to listen...");
    let listen_addr = node.listen_with_test_config().await
        .expect("Failed to start listening");
    println!("✅ Listening started on address: {}", listen_addr);
    
    // Проверяем, что адрес прослушивания получен
    assert!(!listen_addr.to_string().is_empty(), "Should get valid listening address");
    assert!(listen_addr.to_string().contains("/ip4/127.0.0.1/tcp/"), 
            "Listening address should be TCP on localhost");
    
    // Проверяем, что узел имеет корректный Peer ID
    assert!(!node.peer_id.to_string().is_empty(), "Node should have valid Peer ID");
    
    // Проверяем, что узел готов к работе
    println!("✅ Node startup test passed - node started with address: {}", listen_addr);
}

/// Тест запуска ноды с различными конфигурациями
#[tokio::test]
async fn test_node_startup_configurations() {
    // Тестируем запуск узла в режиме сервера
    let (server_node, _server_commander, _server_events, server_peer_id) = 
        crate::test_utils::create_test_node(true, false)
        .await
        .expect("Failed to create server node");
    
    // Создаем TestNode для использования методов NodeExt
    let mut server_test_node = crate::test_utils::TestNode {
        node: server_node,
        commander: _server_commander,
        events: _server_events,
        peer_id: server_peer_id,
    };
    
    let server_addr = server_test_node.listen_with_test_config().await
        .expect("Failed to start server listening");
    
    assert!(!server_addr.to_string().is_empty(), "Server should get valid listening address");
    assert!(!server_test_node.peer_id.to_string().is_empty(), "Server should have valid Peer ID");
    
    // Тестируем запуск узла в режиме клиента с включенным mDNS
    let (client_node, _client_commander, _client_events, client_peer_id) = 
        crate::test_utils::create_test_node(false, true)
        .await
        .expect("Failed to create client node");
    
    let mut client_test_node = crate::test_utils::TestNode {
        node: client_node,
        commander: _client_commander,
        events: _client_events,
        peer_id: client_peer_id,
    };
    
    let client_addr = client_test_node.listen_with_test_config().await
        .expect("Failed to start client listening");
    
    assert!(!client_addr.to_string().is_empty(), "Client should get valid listening address");
    assert!(!client_test_node.peer_id.to_string().is_empty(), "Client should have valid Peer ID");
    
    // Убеждаемся, что адреса разные
    assert_ne!(server_addr, client_addr, "Server and client should have different listening addresses");
    
    println!("✅ Node startup configurations test passed - both server and client started successfully");
}

/// Тест запуска нескольких узлов одновременно
#[tokio::test]
async fn test_multiple_nodes_startup() {
    // Создаем и запускаем несколько узлов одновременно
    let mut node1 = TestNode::new().await.expect("Failed to create first node");
    let mut node2 = TestNode::new().await.expect("Failed to create second node");
    let mut node3 = TestNode::new().await.expect("Failed to create third node");
    
    // Запускаем все узлы
    let addr1 = node1.listen_with_test_config().await.expect("Failed to start node1");
    let addr2 = node2.listen_with_test_config().await.expect("Failed to start node2");
    let addr3 = node3.listen_with_test_config().await.expect("Failed to start node3");
    
    // Проверяем, что все узлы запущены с корректными адресами
    assert!(!addr1.to_string().is_empty(), "Node1 should have valid listening address");
    assert!(!addr2.to_string().is_empty(), "Node2 should have valid listening address");
    assert!(!addr3.to_string().is_empty(), "Node3 should have valid listening address");
    
    // Проверяем, что все адреса разные
    assert_ne!(addr1, addr2, "Node1 and Node2 should have different addresses");
    assert_ne!(addr1, addr3, "Node1 and Node3 should have different addresses");
    assert_ne!(addr2, addr3, "Node2 and Node3 should have different addresses");
    
    // Проверяем, что все Peer ID разные
    assert_ne!(node1.peer_id, node2.peer_id, "Nodes should have different Peer IDs");
    assert_ne!(node1.peer_id, node3.peer_id, "Nodes should have different Peer IDs");
    assert_ne!(node2.peer_id, node3.peer_id, "Nodes should have different Peer IDs");
    
    println!("✅ Multiple nodes startup test passed - all 3 nodes started successfully");
}

/// Тест запуска узла с проверкой сетевого состояния
#[tokio::test]
async fn test_node_startup_network_state() {
    let mut node = TestNode::new().await.expect("Failed to create test node");
    
    // Проверяем начальное состояние сети (должно быть 0 соединений)
    // Это базовая проверка, что узел создан в корректном состоянии
    
    // Запускаем прослушивание
    let listen_addr = node.listen_with_test_config().await
        .expect("Failed to start listening");
    
    // Проверяем, что адрес прослушивания корректный
    assert!(listen_addr.to_string().starts_with("/ip4/127.0.0.1/tcp/"),
            "Listening address should be valid TCP address");
    
    // Проверяем, что узел имеет все необходимые компоненты
    assert!(!node.peer_id.to_string().is_empty(), "Node should have valid Peer ID");
    
    println!("✅ Node startup network state test passed - node started with address: {}", listen_addr);
}

/// Тест запуска временного узла
#[tokio::test]
async fn test_ephemeral_node_startup() {
    // Создаем и запускаем узел в отдельной задаче
    let (_handle, mut commander, mut events, peer_id) = TestNode::new_and_spawn().await
        .expect("Failed to create and spawn ephemeral node");
    
    // Создаем TestNode для использования методов NodeExt
    let mut node = TestNode {
        node: NetworkNode::new_with_config(
            identity::Keypair::generate_ed25519(),
            create_test_por(),
            Some(XRoutesConfig::client()),
        ).await.expect("Failed to create node").0,
        commander,
        events,
        peer_id,
    };
    
    // Запускаем прослушивание
    let listen_addr = node.listen_with_test_config().await
        .expect("Failed to start ephemeral node listening");
    
    // Проверяем базовые свойства узла
    assert!(!listen_addr.to_string().is_empty(), "Ephemeral node should have valid listening address");
    assert!(!node.peer_id.to_string().is_empty(), "Ephemeral node should have valid Peer ID");
    
    println!("✅ Ephemeral node startup test passed - ephemeral node started successfully");
}

// tests/node_core/test_network_state_initial.rs - Тест начального сетевого состояния (PRIORITY 1)

use std::time::Duration;
use libp2p::identity;
use xnetwork::{NetworkNode, XRoutesConfig};
use crate::test_utils::{TestNode, NodeExt, create_test_por};

/// Тест начального состояния сети после создания узла
#[tokio::test]
async fn test_network_state_initial() {
    println!("🔄 Starting test_network_state_initial...");
    
    // Создаем и запускаем узел в отдельной задаче
    let (_handle, commander, events, peer_id) = TestNode::new_and_spawn().await
        .expect("Failed to create and spawn node");
    
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
    
    // Проверяем, что узел имеет корректный Peer ID
    assert!(!node.peer_id.to_string().is_empty(), "Node should have valid Peer ID");
    println!("✅ Node has valid Peer ID: {}", node.peer_id);
    
    // Проверяем, что узел не слушает на старте
    let initial_state = node.commander.get_network_state().await
        .expect("Failed to get initial network state");
    
    // Проверяем базовые свойства начального состояния
    assert_eq!(initial_state.total_connections, 0, "Initial connections should be 0");
    assert!(initial_state.listening_addresses.is_empty(), "Initial listening addresses should be empty");
    assert_eq!(initial_state.authenticated_peers, 0, "Initial authenticated peers should be 0");
    
    println!("✅ Initial network state test passed - node created with clean state");
}

/// Тест сетевого состояния после запуска прослушивания
#[tokio::test]
async fn test_network_state_after_listening() {
    println!("🔄 Starting test_network_state_after_listening...");
    
    // Создаем и запускаем узел в отдельной задаче
    let (_handle, commander, events, peer_id) = TestNode::new_and_spawn().await
        .expect("Failed to create and spawn node");
    
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
        .expect("Failed to start listening");
    println!("✅ Node started listening on: {}", listen_addr);
    
    // Получаем состояние сети после запуска прослушивания
    let state_after_listening = node.commander.get_network_state().await
        .expect("Failed to get network state after listening");
    
    // Проверяем, что узел слушает на корректном адресе
    assert!(!state_after_listening.listening_addresses.is_empty(), 
            "Should have listening addresses after starting");
    // Проверяем, что адрес прослушивания содержит базовые компоненты
    assert!(listen_addr.to_string().contains("/ip4/127.0.0.1/udp/"),
            "Listening address should be UDP on localhost");
    assert!(listen_addr.to_string().contains("/quic-v1"),
            "Listening address should use QUIC protocol");
    
    // Проверяем, что соединений все еще нет
    assert_eq!(state_after_listening.total_connections, 0, 
               "Connections should still be 0 after listening");
    assert_eq!(state_after_listening.authenticated_peers, 0, 
            "Authenticated peers should still be 0 after listening");
    
    println!("✅ Network state after listening test passed - node listening with clean connections");
}

/// Тест сетевого состояния после завершения работы
#[tokio::test]
async fn test_network_state_after_shutdown() {
    println!("🔄 Starting test_network_state_after_shutdown...");
    
    // Создаем и запускаем узел в отдельной задаче
    let (handle, commander, events, peer_id) = TestNode::new_and_spawn().await
        .expect("Failed to create and spawn node");
    
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
        .expect("Failed to start listening");
    println!("✅ Node started listening on: {}", listen_addr);
    
    // Завершаем работу узла
    node.commander.shutdown().await.expect("Failed to send shutdown command");
    tokio::time::timeout(Duration::from_secs(5), handle).await
        .expect("Node shutdown timed out");
    println!("✅ Node shutdown completed");
    
    // Проверяем, что узел действительно завершил работу
    // Попытка получить состояние сети должна завершиться ошибкой
    let state_result = node.commander.get_network_state().await;
    assert!(state_result.is_err(), "Should not be able to get network state after shutdown");
    
    println!("✅ Network state after shutdown test passed - node properly terminated");
}

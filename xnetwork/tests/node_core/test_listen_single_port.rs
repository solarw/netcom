// tests/node_core/test_listen_single_port.rs - Тест прослушивания одного порта (PRIORITY 1)

use std::time::Duration;
use libp2p::identity;
use xnetwork::{NetworkNode, XRoutesConfig};
use crate::test_utils::{TestNode, NodeExt, create_test_por};

/// Тест прослушивания на одном порту с проверкой корректного адреса
#[tokio::test]
async fn test_listen_single_port() {
    println!("🔄 Starting test_listen_single_port...");
    
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
    
    // Проверяем, что адрес прослушивания корректный
    assert!(!listen_addr.to_string().is_empty(), "Listening address should not be empty");
    assert!(listen_addr.to_string().contains("/ip4/127.0.0.1/udp/"), 
            "Listening address should be UDP on localhost");
    assert!(listen_addr.to_string().contains("/quic-v1"), 
            "Listening address should use QUIC protocol");
    assert!(listen_addr.to_string().contains("/p2p/"), 
            "Listening address should include peer ID");
    
    // Проверяем, что адрес содержит корректный Peer ID
    assert!(listen_addr.to_string().contains(&node.peer_id.to_string()),
            "Listening address should contain node's Peer ID");
    
    println!("✅ Single port listening test passed - node listening on valid address");
}

/// Тест получения событий прослушивания
#[tokio::test]
async fn test_listen_address_events() {
    println!("🔄 Starting test_listen_address_events...");
    
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
    
    // Проверяем, что узел действительно слушает
    let state = node.commander.get_network_state().await
        .expect("Failed to get network state");
    
    println!("📊 Network state listening addresses: {:?}", state.listening_addresses);
    println!("📊 Expected listen address: {}", listen_addr);
    
    assert!(!state.listening_addresses.is_empty(), 
            "Node should have listening addresses after starting");
    
    // Проверяем, что есть хотя бы один адрес прослушивания с корректными свойствами
    let has_valid_listening_addr = state.listening_addresses.iter().any(|addr| {
        let addr_str = addr.to_string();
        let is_valid = addr_str.contains("/ip4/127.0.0.1/udp/") && 
                      addr_str.contains("/quic-v1");
        if is_valid {
            println!("✅ Found valid listening address: {}", addr_str);
        }
        is_valid
    });
    assert!(has_valid_listening_addr, 
            "Should have at least one valid listening address");
    
    // Проверяем, что количество прослушивающих адресов равно 1
    assert_eq!(state.listening_addresses.len(), 1, 
               "Should have exactly one listening address");
    
    println!("✅ Listen address events test passed - node properly reporting listening state");
}

/// Тест повторного прослушивания (проверяем, что система корректно обрабатывает множественные вызовы)
#[tokio::test]
async fn test_listen_multiple_calls() {
    println!("🔄 Starting test_listen_multiple_calls...");
    
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
    
    // Первый вызов прослушивания
    let first_addr = node.listen_with_test_config().await
        .expect("Failed to start listening first time");
    println!("✅ First listen call succeeded: {}", first_addr);
    
    // Второй вызов прослушивания (может создать новый слушатель)
    let second_addr = node.listen_with_test_config().await
        .expect("Failed to start listening second time");
    println!("✅ Second listen call succeeded: {}", second_addr);
    
    // Проверяем, что оба адреса корректны
    assert!(first_addr.to_string().contains("/ip4/127.0.0.1/udp/"), 
            "First address should be UDP on localhost");
    assert!(first_addr.to_string().contains("/quic-v1"), 
            "First address should use QUIC protocol");
    assert!(second_addr.to_string().contains("/ip4/127.0.0.1/udp/"), 
            "Second address should be UDP on localhost");
    assert!(second_addr.to_string().contains("/quic-v1"), 
            "Second address should use QUIC protocol");
    
    // Проверяем, что в состоянии есть хотя бы один адрес прослушивания
    let state = node.commander.get_network_state().await
        .expect("Failed to get network state");
    
    assert!(!state.listening_addresses.is_empty(), 
            "Should have at least one listening address after multiple calls");
    
    // Проверяем, что все адреса в состоянии корректны
    let all_addresses_valid = state.listening_addresses.iter().all(|addr| {
        let addr_str = addr.to_string();
        addr_str.contains("/ip4/127.0.0.1/udp/") && addr_str.contains("/quic-v1")
    });
    assert!(all_addresses_valid, 
            "All listening addresses should be valid");
    
    println!("✅ Multiple listen calls test passed - node handles repeated calls correctly");
}

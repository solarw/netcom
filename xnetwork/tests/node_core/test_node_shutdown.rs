// tests/node_core/test_node_shutdown.rs - Тест завершения работы ноды (PRIORITY 1)

use std::time::Duration;
use libp2p::identity;
use xnetwork::{NetworkNode, XRoutesConfig};
use crate::test_utils::{TestNode, NodeExt, create_test_por};

/// Тест корректного завершения работы узла
#[tokio::test]
async fn test_node_shutdown() {
    println!("🔄 Starting test_node_shutdown...");
    
    // Создаем и запускаем узел в отдельной задаче
    let (handle, mut commander, mut events, peer_id) = TestNode::new_and_spawn().await
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
    
    // Отправляем команду завершения работы
    println!("🔄 Sending shutdown command...");
    node.commander.shutdown().await.expect("Failed to send shutdown command");
    println!("✅ Shutdown command sent");
    
    // Ждем завершения задачи узла с таймаутом
    match tokio::time::timeout(Duration::from_secs(5), handle).await {
        Ok(_) => println!("✅ Node shutdown completed successfully"),
        Err(_) => panic!("❌ Node shutdown timed out after 5 seconds"),
    }
    
    println!("✅ Node shutdown test passed - node stopped gracefully");
}

/// Тест завершения работы узла без предварительного запуска
#[tokio::test]
async fn test_node_shutdown_without_startup() {
    println!("🔄 Starting test_node_shutdown_without_startup...");
    
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
    
    // Отправляем команду завершения работы без предварительного запуска
    println!("🔄 Sending shutdown command without startup...");
    node.commander.shutdown().await.expect("Failed to send shutdown command");
    println!("✅ Shutdown command sent");
    
    // Ждем завершения задачи узла с таймаутом
    match tokio::time::timeout(Duration::from_secs(5), handle).await {
        Ok(_) => println!("✅ Node shutdown completed successfully"),
        Err(_) => panic!("❌ Node shutdown timed out after 5 seconds"),
    }
    
    println!("✅ Node shutdown without startup test passed - node stopped gracefully");
}

/// Тест завершения работы нескольких узлов
#[tokio::test]
async fn test_multiple_nodes_shutdown() {
    println!("🔄 Starting test_multiple_nodes_shutdown...");
    
    // Создаем и запускаем несколько узлов
    let (handle1, commander1, events1, peer_id1) = TestNode::new_and_spawn().await
        .expect("Failed to create first node");
    let (handle2, commander2, events2, peer_id2) = TestNode::new_and_spawn().await
        .expect("Failed to create second node");
    
    // Создаем TestNode для использования методов NodeExt
    let mut node1 = TestNode {
        node: NetworkNode::new_with_config(
            identity::Keypair::generate_ed25519(),
            create_test_por(),
            Some(XRoutesConfig::client()),
        ).await.expect("Failed to create node").0,
        commander: commander1,
        events: events1,
        peer_id: peer_id1,
    };
    
    let mut node2 = TestNode {
        node: NetworkNode::new_with_config(
            identity::Keypair::generate_ed25519(),
            create_test_por(),
            Some(XRoutesConfig::client()),
        ).await.expect("Failed to create node").0,
        commander: commander2,
        events: events2,
        peer_id: peer_id2,
    };
    
    // Запускаем прослушивание на обоих узлах
    let addr1 = node1.listen_with_test_config().await.expect("Failed to start node1");
    let addr2 = node2.listen_with_test_config().await.expect("Failed to start node2");
    println!("✅ Both nodes started listening: {} and {}", addr1, addr2);
    
    // Завершаем работу первого узла
    println!("🔄 Shutting down first node...");
    node1.commander.shutdown().await.expect("Failed to shutdown node1");
    match tokio::time::timeout(Duration::from_secs(5), handle1).await {
        Ok(_) => println!("✅ First node shutdown completed"),
        Err(_) => panic!("❌ First node shutdown timed out"),
    }
    
    // Проверяем, что второй узел все еще работает
    println!("🔄 Checking second node is still alive...");
    let addr2_check = node2.listen_with_test_config().await;
    assert!(addr2_check.is_ok(), "Second node should still be running");
    
    // Завершаем работу второго узла
    println!("🔄 Shutting down second node...");
    node2.commander.shutdown().await.expect("Failed to shutdown node2");
    match tokio::time::timeout(Duration::from_secs(5), handle2).await {
        Ok(_) => println!("✅ Second node shutdown completed"),
        Err(_) => panic!("❌ Second node shutdown timed out"),
    }
    
    println!("✅ Multiple nodes shutdown test passed - both nodes stopped gracefully");
}

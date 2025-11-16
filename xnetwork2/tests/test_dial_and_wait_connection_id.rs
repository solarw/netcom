//! Тест для проверки dial_and_wait с возвратом ConnectionId

use std::time::Duration;
use xnetwork2::Node;

#[tokio::test]
async fn test_dial_and_wait_returns_connection_id() {
    // Создаем две ноды
    let mut node1 = Node::new().await.expect("Failed to create node1");
    let mut node2 = Node::new().await.expect("Failed to create node2");
    
    // Запускаем обе ноды
    node1.start().await.expect("Failed to start node1");
    node2.start().await.expect("Failed to start node2");
    
    // Получаем адрес прослушивания второй ноды
    let listen_addr = node2.commander.listen_and_wait(
        "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap(),
        Duration::from_secs(5)
    ).await.expect("Failed to listen on node2");
    
    println!("Node2 listening on: {}", listen_addr);
    
    // Получаем PeerId второй ноды
    let node2_peer_id = node2.commander.get_network_state()
        .await
        .expect("Failed to get node2 network state")
        .peer_id;
    
    println!("Node2 PeerId: {}", node2_peer_id);
    
    // Пытаемся подключиться с использованием dial_and_wait
    let connection_id = node1.commander.dial_and_wait(
        node2_peer_id,
        listen_addr,
        Duration::from_secs(10)
    ).await.expect("Failed to dial node2");
    
    println!("✅ Dial_and_wait успешно вернул ConnectionId: {:?}", connection_id);
    
    // Проверяем, что подключение действительно установлено
    let network_state = node1.commander.get_network_state()
        .await
        .expect("Failed to get node1 network state");
    
    assert!(
        network_state.connected_peers.contains(&node2_peer_id),
        "Node2 should be in connected peers list"
    );
    
    println!("✅ Подключение к Node2 подтверждено в состоянии сети");
    
    // Останавливаем ноды
    node1.commander.shutdown().await.expect("Failed to shutdown node1");
    node2.commander.shutdown().await.expect("Failed to shutdown node2");
    
    println!("✅ Тест завершен успешно - dial_and_wait возвращает ConnectionId");
}

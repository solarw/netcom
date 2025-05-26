// tests/common.rs - Общие функции для интеграционных тестов

use std::time::Duration;
use libp2p::{identity, PeerId};
use xauth::por::por::ProofOfRepresentation;
use xnetwork::{
    commander::Commander,
    events::NetworkEvent,
    node::NetworkNode,
    XRoutesConfig,
};

/// Создает тестовый PoR для тестирования
pub fn create_test_por() -> ProofOfRepresentation {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    
    ProofOfRepresentation::create(
        &keypair,
        peer_id,
        Duration::from_secs(3600), // 1 час валидности
    ).expect("Failed to create test PoR")
}

/// Создает тестовый узел с заданными параметрами
pub async fn create_test_node(
    server_mode: bool,
    enable_mdns: bool,
) -> Result<(NetworkNode, Commander, tokio::sync::mpsc::Receiver<NetworkEvent>, PeerId), Box<dyn std::error::Error + Send + Sync>> {
    let key = identity::Keypair::generate_ed25519();
    let por = create_test_por();
    
    let (node, cmd_tx, event_rx, peer_id) = NetworkNode::new(
        key,
        por,
        enable_mdns,
        server_mode,
    ).await?;

    let commander = Commander::new(cmd_tx);
    
    Ok((node, commander, event_rx, peer_id))
}

/// Создает тестовый узел с расширенной конфигурацией
pub async fn create_test_node_with_config(
    config: XRoutesConfig,
) -> Result<(NetworkNode, Commander, tokio::sync::mpsc::Receiver<NetworkEvent>, PeerId), Box<dyn std::error::Error + Send + Sync>> {
    let key = identity::Keypair::generate_ed25519();
    let por = create_test_por();
    
    let (node, cmd_tx, event_rx, peer_id) = NetworkNode::new_with_config(
        key,
        por,
        Some(config),
    ).await?;

    let commander = Commander::new(cmd_tx);
    
    Ok((node, commander, event_rx, peer_id))
}

/// Создает bootstrap сервер для тестов
pub async fn create_bootstrap_server() -> Result<(tokio::task::JoinHandle<()>, libp2p::Multiaddr, PeerId), Box<dyn std::error::Error + Send + Sync>> {
    let (mut bootstrap_node, bootstrap_commander, mut bootstrap_events, bootstrap_peer_id) = 
        create_test_node(true, false).await?;

    // Запускаем узел в отдельной задаче
    let node_handle = tokio::task::spawn(async move {
        bootstrap_node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });

    // Даем время узлу стабилизироваться
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Запускаем слушатель
    bootstrap_commander
        .listen_port(Some("127.0.0.1".to_string()), 0)
        .await?;

    // Ждем адрес прослушивания
    let listen_addr = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = bootstrap_events.recv().await {
            if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                return addr;
            }
        }
        panic!("No listening address received");
    }).await?;

    Ok((node_handle, listen_addr, bootstrap_peer_id))
}

/// Очистка ресурсов после тестов
pub async fn cleanup_test_nodes(handles: Vec<tokio::task::JoinHandle<()>>) {
    for handle in handles {
        handle.abort();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
}
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
#[allow(dead_code)]
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
        None, // cmd_queue_size
        None, // event_queue_size
    ).await?;

    let commander = Commander::new(cmd_tx);
    
    Ok((node, commander, event_rx, peer_id))
}

/// Создает bootstrap сервер для тестов
#[allow(dead_code)]
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
#[allow(dead_code)]
pub async fn cleanup_test_nodes(handles: Vec<tokio::task::JoinHandle<()>>) {
    for handle in handles {
        handle.abort();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
}

/// Ожидает события подключения peer'а
#[allow(dead_code)]
pub async fn wait_for_peer_connection(
    events: &mut tokio::sync::mpsc::Receiver<NetworkEvent>,
    target_peer_id: PeerId,
    timeout_secs: u64,
) -> bool {
    tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        while let Some(event) = events.recv().await {
            if let NetworkEvent::PeerConnected { peer_id } = event {
                if peer_id == target_peer_id {
                    return true;
                }
            }
        }
        false
    }).await.unwrap_or(false)
}

/// Ожидает события отключения peer'а
#[allow(dead_code)]
pub async fn wait_for_peer_disconnection(
    events: &mut tokio::sync::mpsc::Receiver<NetworkEvent>,
    target_peer_id: PeerId,
    timeout_secs: u64,
) -> bool {
    tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        while let Some(event) = events.recv().await {
            if let NetworkEvent::PeerDisconnected { peer_id } = event {
                if peer_id == target_peer_id {
                    return true;
                }
            }
        }
        false
    }).await.unwrap_or(false)
}

/// Проверяет что peer подключен
#[allow(dead_code)]
pub async fn assert_peer_connected(
    commander: &Commander,
    peer_id: PeerId,
) -> Result<(), String> {
    let peer_info = commander.get_peer_info(peer_id).await
        .map_err(|e| format!("Failed to get peer info: {}", e))?;
    
    match peer_info {
        Some(info) if info.is_connected() => Ok(()),
        Some(info) => Err(format!("Peer {} is not connected (has {} connections)", 
                                 peer_id, info.connection_count())),
        None => Err(format!("Peer {} not found", peer_id)),
    }
}

/// Проверяет что соединение существует
#[allow(dead_code)]
pub async fn assert_connection_exists(
    commander: &Commander,
    connection_id: libp2p::swarm::ConnectionId,
) -> Result<(), String> {
    let connection_info = commander.get_connection_info(connection_id).await
        .map_err(|e| format!("Failed to get connection info: {}", e))?;
    
    match connection_info {
        Some(info) if info.is_active() => Ok(()),
        Some(info) => Err(format!("Connection {:?} exists but is not active: {:?}", 
                                 connection_id, info.connection_state)),
        None => Err(format!("Connection {:?} not found", connection_id)),
    }
}

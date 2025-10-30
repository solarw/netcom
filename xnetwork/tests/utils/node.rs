//! Утилиты для создания и управления тестовыми нодами

use std::time::Duration;
use libp2p::{identity, Multiaddr, PeerId};
use tokio::sync::mpsc;
use xauth::por::por::ProofOfRepresentation;
use xnetwork::{
    commander::Commander,
    events::NetworkEvent,
    NodeBuilder,
    XRoutesConfig,
};

/// Утилиты для работы с NetID
pub struct NetIDUtils;

impl NetIDUtils {
    /// Создает тестовый PoR
    pub fn create_test_por() -> ProofOfRepresentation {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        
        ProofOfRepresentation::create(
            &keypair,
            peer_id,
            Duration::from_secs(3600), // 1 час валидности
        ).expect("Failed to create test PoR")
    }
    
}

/// Создает тестовый PoR для тестирования
pub fn create_test_por() -> ProofOfRepresentation {
    NetIDUtils::create_test_por()
}

/// Простая функция для создания и запуска ноды
/// Возвращает: (commander, events, node_handle, peer_id)
pub async fn create_node() -> Result<
    (
        Commander, 
        mpsc::Receiver<NetworkEvent>, 
        tokio::task::JoinHandle<()>, 
        PeerId
    ), 
    Box<dyn std::error::Error + Send + Sync>
> {
    let key = identity::Keypair::generate_ed25519();
    let por = create_test_por();
    
    let builder = NodeBuilder::new(key, por)
        .with_config(XRoutesConfig::client());
    
    let (mut node, cmd_tx, event_rx, peer_id) = builder.build().await?;
    let commander = Commander::new(cmd_tx);
    
    let node_handle = tokio::spawn(async move {
        node.run_with_cleanup_interval(Duration::from_secs(1)).await;
    });
    
    Ok((commander, event_rx, node_handle, peer_id))
}

/// Создает тестовый узел с заданными параметрами через NodeBuilder
pub async fn create_test_node(
    server_mode: bool,
    enable_mdns: bool,
) -> Result<(xnetwork::node::NetworkNode, Commander, mpsc::Receiver<NetworkEvent>, PeerId), Box<dyn std::error::Error + Send + Sync>> {
    let key = identity::Keypair::generate_ed25519();
    let por = create_test_por();
    
    let builder = NodeBuilder::new(key, por)
        .enable_mdns()
        .server_mode();
    
    let (node, cmd_tx, event_rx, peer_id) = builder.build().await?;
    let commander = Commander::new(cmd_tx);
    
    Ok((node, commander, event_rx, peer_id))
}

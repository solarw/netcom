// tests/test_utils.rs - Расширенная тестовая оснастка для xnetwork

use std::time::Duration;
use libp2p::{identity, Multiaddr, PeerId};
use tokio::sync::mpsc;
use xauth::por::por::ProofOfRepresentation;
use xnetwork::{
    commander::Commander,
    events::NetworkEvent,
    node::NetworkNode,
    XRoutesConfig,
};

/// Трейт расширения для удобного тестирования узлов
pub trait NodeExt {
    /// Создает временный узел для тестирования
    async fn new_ephemeral() -> Result<TestNode, Box<dyn std::error::Error + Send + Sync>>;
    
    /// Запускает прослушивание с тестовой конфигурацией
    async fn listen_with_test_config(&mut self) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>>;
    
    /// Подключается к другому узлу
    async fn connect_to_peer(&mut self, peer_addr: Multiaddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Ожидает конкретное событие
    async fn wait_for_event<F>(&mut self, predicate: F, timeout: Duration) -> Option<NetworkEvent>
        where F: Fn(&NetworkEvent) -> bool;
    
    /// Ожидает подключение к конкретному пиру
    async fn wait_for_connection(&mut self, peer_id: PeerId, timeout: Duration) -> bool;
    
    /// Ожидает адрес прослушивания
    async fn wait_for_listening_addr(&mut self, timeout: Duration) -> Option<Multiaddr>;
}

/// Тестовый узел с удобным интерфейсом
pub struct TestNode {
    pub node: NetworkNode,
    pub commander: Commander,
    pub events: mpsc::Receiver<NetworkEvent>,
    pub peer_id: PeerId,
}

impl TestNode {
    /// Создает новый тестовый узел (без запуска)
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key = identity::Keypair::generate_ed25519();
        let por = create_test_por();
        
        let (node, cmd_tx, event_rx, peer_id) = NetworkNode::new_with_config(
            key,
            por,
            Some(XRoutesConfig::client()),
        ).await?;

        let commander = Commander::new(cmd_tx);
        
        Ok(Self {
            node,
            commander,
            events: event_rx,
            peer_id,
        })
    }
    
    /// Запускает узел в отдельной задаче
    pub fn spawn(mut self) -> (tokio::task::JoinHandle<()>, Commander, mpsc::Receiver<NetworkEvent>, PeerId) {
        let peer_id = self.peer_id;
        let commander = self.commander;
        let events = self.events;
        
        let handle = tokio::spawn(async move {
            println!("🚀 Starting network node...");
            self.node.run_with_cleanup_interval(Duration::from_secs(1)).await;
            println!("🛑 Network node stopped");
        });
        
        (handle, commander, events, peer_id)
    }
    
    /// Создает и сразу запускает узел
    pub async fn new_and_spawn() -> Result<(tokio::task::JoinHandle<()>, Commander, mpsc::Receiver<NetworkEvent>, PeerId), Box<dyn std::error::Error + Send + Sync>> {
        let node = Self::new().await?;
        Ok(node.spawn())
    }
}

impl NodeExt for TestNode {
    async fn new_ephemeral() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new().await
    }
    
    async fn listen_with_test_config(&mut self) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
        println!("🔄 Sending listen_port command...");
        self.commander.listen_port(Some("127.0.0.1".to_string()), 0).await?;
        println!("✅ listen_port command sent successfully");
        
        println!("🔄 Waiting for listening address...");
        match self.wait_for_listening_addr(Duration::from_secs(5)).await {
            Some(addr) => {
                println!("✅ Got listening address: {}", addr);
                Ok(addr)
            }
            None => {
                println!("❌ Timeout waiting for listening address");
                Err("Failed to get listening address within timeout".into())
            }
        }
    }
    
    async fn connect_to_peer(&mut self, peer_addr: Multiaddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.connect(peer_addr).await?;
        Ok(())
    }
    
    async fn wait_for_event<F>(&mut self, predicate: F, timeout: Duration) -> Option<NetworkEvent>
        where F: Fn(&NetworkEvent) -> bool
    {
        match tokio::time::timeout(timeout, async {
            while let Some(event) = self.events.recv().await {
                println!("📡 Received event: {:?}", event);
                if predicate(&event) {
                    return Some(event);
                }
            }
            None
        }).await {
            Ok(result) => result,
            Err(_) => {
                println!("⏰ Timeout waiting for event after {:?}", timeout);
                None
            }
        }
    }
    
    async fn wait_for_connection(&mut self, peer_id: PeerId, timeout: Duration) -> bool {
        self.wait_for_event(|event| {
            matches!(event, NetworkEvent::PeerConnected { peer_id: id } if *id == peer_id)
        }, timeout).await.is_some()
    }
    
    async fn wait_for_listening_addr(&mut self, timeout: Duration) -> Option<Multiaddr> {
        self.wait_for_event(|event| {
            matches!(event, NetworkEvent::ListeningOnAddress { full_addr: Some(_), .. })
        }, timeout).await.and_then(|event| {
            if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                Some(addr)
            } else {
                None
            }
        })
    }
}

/// Ожидатель событий для удобного тестирования
pub struct EventWaiter<'a> {
    events: &'a mut mpsc::Receiver<NetworkEvent>,
}

impl<'a> EventWaiter<'a> {
    pub fn new(events: &'a mut mpsc::Receiver<NetworkEvent>) -> Self {
        Self { events }
    }
    
    /// Ожидает подключение пира
    pub async fn wait_for_peer_connected(&mut self, peer_id: PeerId, timeout: Duration) -> bool {
        tokio::time::timeout(timeout, async {
            while let Some(event) = self.events.recv().await {
                if let NetworkEvent::PeerConnected { peer_id: id } = event {
                    if id == peer_id {
                        return true;
                    }
                }
            }
            false
        }).await.unwrap_or(false)
    }
    
    /// Ожидает отключение пира
    pub async fn wait_for_peer_disconnected(&mut self, peer_id: PeerId, timeout: Duration) -> bool {
        tokio::time::timeout(timeout, async {
            while let Some(event) = self.events.recv().await {
                if let NetworkEvent::PeerDisconnected { peer_id: id } = event {
                    if id == peer_id {
                        return true;
                    }
                }
            }
            false
        }).await.unwrap_or(false)
    }
    
    /// Ожидает адрес прослушивания
    pub async fn wait_for_listening_addr(&mut self, timeout: Duration) -> Option<Multiaddr> {
        tokio::time::timeout(timeout, async {
            while let Some(event) = self.events.recv().await {
                if let NetworkEvent::ListeningOnAddress { full_addr: Some(addr), .. } = event {
                    return Some(addr);
                }
            }
            None
        }).await.ok().flatten()
    }
}

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
    
    /// Создает несколько узлов с одним NetID (мульти-устройственная сущность)
    pub async fn create_multi_device_entity(
        count: usize,
    ) -> Result<Vec<TestNode>, Box<dyn std::error::Error + Send + Sync>> {
        let mut nodes = Vec::with_capacity(count);
        
        for _ in 0..count {
            let node = TestNode::new().await?;
            nodes.push(node);
        }
        
        Ok(nodes)
    }
}

/// Помощник для создания соединенных пар узлов
pub struct ConnectionHelper;

impl ConnectionHelper {
    /// Создает пару соединенных узлов
    pub async fn create_connected_pair() -> Result<(TestNode, TestNode), Box<dyn std::error::Error + Send + Sync>> {
        let mut server = TestNode::new().await?;
        let mut client = TestNode::new().await?;
        
        // Запускаем сервер
        let server_addr = server.listen_with_test_config().await?;
        
        // Подключаем клиент
        client.connect_to_peer(server_addr.clone()).await?;
        
        // Ждем установления соединения с таймаутом
        let connected = tokio::time::timeout(
            Duration::from_secs(10),
            async {
                client.wait_for_connection(server.peer_id, Duration::from_secs(5)).await
            }
        ).await.unwrap_or(false);
        
        if !connected {
            return Err("Failed to establish connection within timeout".into());
        }
        
        Ok((server, client))
    }
    
    /// Создает полностью соединенную сеть
    pub async fn create_fully_connected_mesh(
        count: usize,
    ) -> Result<Vec<TestNode>, Box<dyn std::error::Error + Send + Sync>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        
        let mut nodes = Vec::with_capacity(count);
        
        // Создаем первый узел
        let mut first_node = TestNode::new().await?;
        let first_addr = first_node.listen_with_test_config().await?;
        nodes.push(first_node);
        
        // Создаем и подключаем остальные узлы
        for _ in 1..count {
            let mut node = TestNode::new().await?;
            node.connect_to_peer(first_addr.clone()).await?;
            
            // Ждем установления соединения с таймаутом
            let connected = tokio::time::timeout(
                Duration::from_secs(10),
                async {
                    node.wait_for_connection(nodes[0].peer_id, Duration::from_secs(5)).await
                }
            ).await.unwrap_or(false);
            
            if !connected {
                return Err("Failed to establish connection within timeout".into());
            }
            
            nodes.push(node);
        }
        
        Ok(nodes)
    }
}

/// Создает тестовый PoR для тестирования
pub fn create_test_por() -> ProofOfRepresentation {
    NetIDUtils::create_test_por()
}

/// Создает тестовый узел с заданными параметрами
pub async fn create_test_node(
    server_mode: bool,
    enable_mdns: bool,
) -> Result<(NetworkNode, Commander, mpsc::Receiver<NetworkEvent>, PeerId), Box<dyn std::error::Error + Send + Sync>> {
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

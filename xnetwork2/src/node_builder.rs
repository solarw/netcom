//! NodeBuilder для конфигурируемого создания Node в XNetwork2
//!
//! Поддерживает fluent интерфейс для настройки поведения узла,
//! включая политику принятия решений для входящих XStream потоков.
use std::time::Duration;
use libp2p::{identity, quic};
use tokio::sync::broadcast;
use xstream::events::IncomingConnectionApprovePolicy;

/// Политика принятия решений для входящих потоков
#[derive(Debug, Clone, Copy)]
pub enum InboundDecisionPolicy {
    /// Передавать события для ручного принятия решений через NodeEvent
    ManualApprove,
}

impl Default for InboundDecisionPolicy {
    fn default() -> Self {
        Self::ManualApprove
    }
}

/// Конфигурация для создания Node
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Политика принятия решений для входящих потоков
    pub inbound_decision_policy: InboundDecisionPolicy,
    /// Размер буфера для каналов событий
    pub event_buffer_size: usize,
    /// Включить relay сервер
    pub enable_relay_server: bool,
    /// Включить DCUtR для hole punching
    pub enable_dcutr: bool,
    /// Включить AutoNAT для определения типа NAT
    pub enable_autonat: bool,
    /// Включить Kademlia DHT discovery
    pub enable_kademlia: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            inbound_decision_policy: InboundDecisionPolicy::default(),
            event_buffer_size: 100,
            enable_relay_server: false,
            enable_dcutr: false,
            enable_autonat: false,
            enable_kademlia: false,
        }
    }
}

/// Fluent builder для создания конфигурируемого Node
pub struct NodeBuilder {
    config: NodeConfig,
    keypair: Option<identity::Keypair>,
}

impl NodeBuilder {
    /// Создает новый NodeBuilder с конфигурацией по умолчанию
    pub fn new() -> Self {
        Self {
            config: NodeConfig::default(),
            keypair: None,
        }
    }

    /// Устанавливает политику принятия решений для входящих потоков
    pub fn with_inbound_decision_policy(mut self, policy: InboundDecisionPolicy) -> Self {
        self.config.inbound_decision_policy = policy;
        self
    }

    /// Устанавливает размер буфера для каналов событий
    pub fn with_event_buffer_size(mut self, size: usize) -> Self {
        self.config.event_buffer_size = size;
        self
    }

    /// Устанавливает пользовательский ключ для узла
    pub fn with_keypair(mut self, keypair: identity::Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Устанавливает фиксированный приватный ключ из байтов (Ed25519)
    pub fn with_fixed_key(mut self, key_bytes: Vec<u8>) -> Self {
        use libp2p::identity::ed25519;
        
        if key_bytes.len() == 32 {
            // Создаем ключ из 32-байтного seed используя правильный API
            let mut seed_copy = key_bytes.clone();
            match ed25519::SecretKey::try_from_bytes(&mut seed_copy) {
                Ok(secret_key) => {
                    let ed25519_keypair = ed25519::Keypair::from(secret_key);
                    self.keypair = Some(identity::Keypair::from(ed25519_keypair));
                    println!("✅ Fixed key loaded successfully from 32-byte seed");
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to create keypair from 32-byte seed: {}", e);
                    // Fallback to generated key
                    self.keypair = Some(identity::Keypair::generate_ed25519());
                }
            }
        } else {
            eprintln!("⚠️ Invalid key length: {} bytes, expected 32", key_bytes.len());
            // Fallback to generated key
            self.keypair = Some(identity::Keypair::generate_ed25519());
        }
        self
    }

    /// Устанавливает конфигурацию XRoutes
    pub fn with_xroutes_config<F>(mut self, config_fn: F) -> Self
    where
        F: FnOnce(crate::behaviours::xroutes::XRoutesConfig) -> crate::behaviours::xroutes::XRoutesConfig,
    {
        // Note: XRoutesConfig is currently used in XRoutesHandler, not in NodeBuilder
        // This method is provided for future compatibility
        self
    }

    /// Включает relay сервер
    pub fn with_relay_server(mut self) -> Self {
        self.config.enable_relay_server = true;
        self
    }

    /// Включает DCUtR для hole punching
    pub fn with_dcutr(mut self) -> Self {
        self.config.enable_dcutr = true;
        self
    }

    /// Включает AutoNAT для определения типа NAT
    pub fn with_autonat(mut self) -> Self {
        self.config.enable_autonat = true;
        self
    }

    /// Включает все механизмы NAT traversal (relay, DCUtR, AutoNAT)
    pub fn with_nat_traversal(mut self) -> Self {
        self.config.enable_relay_server = true;
        self.config.enable_dcutr = true;
        self.config.enable_autonat = true;
        self
    }

    /// Включает Kademlia DHT discovery
    pub fn with_kademlia(mut self) -> Self {
        self.config.enable_kademlia = true;
        self
    }

    /// Создает Node с текущей конфигурацией
    pub async fn build(
        self,
    ) -> Result<crate::node::Node, Box<dyn std::error::Error + Send + Sync>> {
        use crate::node::Node;

        println!(
            "🛠️ Building XNetwork2 node with configuration: {:?}",
            self.config
        );

        // Создаем или используем существующий ключ
        let keypair = self
            .keypair
            .unwrap_or_else(|| identity::Keypair::generate_ed25519());
        let peer_id = keypair.public().to_peer_id();
        println!("🔑 Generated/using keypair with PeerId: {}", peer_id);
        
        // Создаем QUIC транспорт
        let quic_config = quic::Config::new(&keypair);
        let quic_transport = quic::tokio::Transport::new(quic_config);

        // Определяем политику для XStream - всегда ручной контроль через события
        let xstream_policy = IncomingConnectionApprovePolicy::ApproveViaEvent;

        // Создаем swarm с XStream поведением с выбранной политикой
        let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_other_transport(|_key| quic_transport)
            .expect("Failed to create QUIC transport")
            .with_relay_client(libp2p::noise::Config::new, libp2p::yamux::Config::default)
            .expect("Failed to create relay client transport")
            .with_behaviour(|key, relay_client_behaviour| {
                let peer_id = key.public().to_peer_id();

                let ping_config = libp2p::ping::Config::new()
                    .with_interval(Duration::from_secs(1))
                    ; // держать соединение активным
                let ping_behaviour = libp2p::ping::Behaviour::new(ping_config);

                // Безопасное создание POR
                let por = xauth::por::por::ProofOfRepresentation::create(
                    &key,
                    peer_id,
                    std::time::Duration::from_secs(3600), // 1 hour validity
                ).expect("❌ CRITICAL SECURITY ERROR: Failed to create Proof of Representation - system security compromised");

                let xauth_behaviour = xauth::behaviours::PorAuthBehaviour::new(por);

                let xstream_behaviour = xstream::behaviour::XStreamNetworkBehaviour::new_with_policy(xstream_policy);

        // Create XRoutes behaviour with NAT traversal configuration
        let xroutes_config = crate::behaviours::xroutes::XRoutesConfig::disabled()
            .with_relay_server(self.config.enable_relay_server)
            .with_dcutr(self.config.enable_dcutr)
            .with_autonat(self.config.enable_autonat)
            .with_kad(self.config.enable_kademlia)
            .with_identify(true);
        let xroutes_behaviour = crate::behaviours::xroutes::XRoutesBehaviour::new(
            keypair.public(),
            &xroutes_config,
            Some(relay_client_behaviour), // Pass the relay client behaviour as Some
        ).expect("Failed to create XRoutes behaviour");

                // Create KeepAlive behaviour
                let keep_alive_behaviour = crate::behaviours::keep_alive::KeepAliveBehaviour::new();

                // Create main behaviour
                crate::main_behaviour::XNetworkBehaviour {
                    ping: ping_behaviour,
                    xauth: xauth_behaviour,
                    xstream: xstream_behaviour,
                    xroutes: xroutes_behaviour,
                    keep_alive: keep_alive_behaviour,
                }
            })
            .unwrap()
            .build();

        let peer_id = swarm.local_peer_id().clone();
        println!("🆕 XNetwork2 node created with PeerId: {}", peer_id);

        // Create broadcast channel for NodeEvents
        let (event_sender, _) = broadcast::channel(self.config.event_buffer_size);

        // Create handler dispatcher with event channel
        let behaviour_handler_dispatcher =
            crate::main_behaviour::XNetworkBehaviourHandlerDispatcher {
                swarm_handler: crate::swarm_handler::XNetworkSwarmHandler::with_event_sender(
                    event_sender.clone(),
                ),
                //identify: crate::behaviours::IdentifyHandler::default(),
                ping: crate::behaviours::PingHandler::default(),
                xauth: crate::behaviours::XAuthHandler::default(),
                xstream: crate::behaviours::XStreamHandler::default(),
                xroutes: crate::behaviours::XRoutesHandler::new(
                    keypair.public(),
                    crate::behaviours::xroutes::XRoutesConfig::default(),
                ),
                keep_alive: crate::behaviours::KeepAliveHandler::default(),
            };

        // Create SwarmLoop using correct builder pattern
        let sl2_builder: command_swarm::SwarmLoopBuilder<
            crate::main_behaviour::XNetworkBehaviour,
            crate::main_behaviour::XNetworkBehaviourHandlerDispatcher,
            crate::main_behaviour::XNetworkCommands,
        > = command_swarm::SwarmLoopBuilder::new()
            .with_behaviour_handler(behaviour_handler_dispatcher)
            .with_channel_size(self.config.event_buffer_size)
            .with_swarm(swarm);

        let (command_tx, stopper, swarm_loop) = sl2_builder.build().unwrap();

        // Create commander wrapper
        let commander = crate::commander::Commander::new(command_tx.clone(), stopper.clone());

        Ok(Node {
            command_tx,
            commander,
            stopper,
            swarm_loop: Some(swarm_loop),
            swarm_loop_handle: None,
            event_sender,
            peer_id,
            keypair,
        })
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Упрощенный конструктор для обратной совместимости
pub fn builder() -> NodeBuilder {
    NodeBuilder::new()
}

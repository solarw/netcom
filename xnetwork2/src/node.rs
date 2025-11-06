//! Node creation and management for XNetwork2

use libp2p::{identity, quic};
use command_swarm::{SwarmLoop, SwarmLoopBuilder, SwarmLoopStopper};
use tokio::sync::{mpsc, broadcast};

use crate::node_events::NodeEvent;

use crate::main_behaviour::{XNetworkBehaviour, XNetworkCommands, XNetworkBehaviourHandlerDispatcher};
use crate::swarm_handler::XNetworkSwarmHandler;
use crate::behaviours::{IdentifyHandler, PingHandler, XAuthHandler, XStreamHandler};
use crate::commander::Commander;

/// XNetwork2 Node
pub struct Node {
    /// Commander for sending commands to the node
    pub command_tx: mpsc::Sender<XNetworkCommands>,
    /// Commander wrapper for convenient command sending with responses
    pub commander: Commander,
    /// Stopper for graceful shutdown
    pub stopper: SwarmLoopStopper,
    /// SwarmLoop before starting (None after start)
    pub swarm_loop: Option<SwarmLoop<XNetworkBehaviour, XNetworkBehaviourHandlerDispatcher, XNetworkCommands>>,
    /// Handle to the background swarm loop task (Some after start)
    pub swarm_loop_handle: Option<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
    /// Broadcast channel for sending NodeEvents to multiple subscribers
    pub event_sender: broadcast::Sender<NodeEvent>,
    /// Peer ID of this node (available immediately after creation)
    pub peer_id: libp2p::PeerId,
    /// Keypair used by this node (available immediately after creation)
    pub keypair: identity::Keypair,
}

impl Node {
    /// Create a new XNetwork2 node
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        println!("🆕 Creating XNetwork2 node with QUIC transport...");

        // ✅ Явное создание ключа для всей системы
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();
        println!("🔑 Generated keypair with PeerId: {}", peer_id);

        // Create QUIC transport с явным ключом
        let quic_config = quic::Config::new(&keypair);
        let quic_transport = quic::tokio::Transport::new(quic_config);

        // Create swarm using explicit identity
        let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_other_transport(|_key| quic_transport)
            .expect("Failed to create QUIC transport")
            .with_behaviour(|key| {
                let peer_id = key.public().to_peer_id();
                
                // Create behaviours
                let identify_behaviour = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "/xnetwork2/1.0.0".to_string(),
                    key.public(),
                ));
                
                let ping_behaviour = libp2p::ping::Behaviour::new(libp2p::ping::Config::new());
                
                // ✅ КРИТИЧЕСКОЕ ИСПРАВЛЕНИЕ: Безопасное создание POR
                let por = xauth::por::por::ProofOfRepresentation::create(
                    &key,
                    peer_id,
                    std::time::Duration::from_secs(3600), // 1 hour validity
                ).expect("❌ CRITICAL SECURITY ERROR: Failed to create Proof of Representation - system security compromised");
                
                let xauth_behaviour = xauth::behaviours::PorAuthBehaviour::new(por);
                
                let xstream_behaviour = xstream::behaviour::XStreamNetworkBehaviour::new();

                // Create main behaviour
                XNetworkBehaviour {
                    identify: identify_behaviour,
                    ping: ping_behaviour,
                    xauth: xauth_behaviour,
                    xstream: xstream_behaviour,
                }
            })
            .unwrap()
            .build();

        let peer_id = swarm.local_peer_id().clone();
        println!("🆕 XNetwork2 node created with PeerId: {}", peer_id);

        // Create broadcast channel for NodeEvents
        let (event_sender, _) = broadcast::channel(32); // buffer size 32

        // Create handler dispatcher with event channel
        let behaviour_handler_dispatcher = XNetworkBehaviourHandlerDispatcher {
            swarm_handler: XNetworkSwarmHandler::with_event_sender(event_sender.clone()),
            identify: IdentifyHandler::default(),
            ping: PingHandler::default(),
            xauth: XAuthHandler::default(),
            xstream: XStreamHandler::default(),
        };

        // Create SwarmLoop using correct builder pattern
        let sl2_builder: SwarmLoopBuilder<XNetworkBehaviour, XNetworkBehaviourHandlerDispatcher, XNetworkCommands> =
            SwarmLoopBuilder::new()
                .with_behaviour_handler(behaviour_handler_dispatcher)
                .with_channel_size(32)
                .with_swarm(swarm);
        
        let (command_tx, stopper, swarm_loop) = sl2_builder.build().unwrap();

        // Create commander wrapper
        let commander = Commander::new(command_tx.clone(), stopper.clone());

        Ok(Node {
            command_tx: command_tx,
            commander,
            stopper,
            swarm_loop: Some(swarm_loop),
            swarm_loop_handle: None,
            event_sender,
            peer_id,
            keypair,
        })
    }

    /// Start the node
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🚀 Starting XNetwork2 node...");
        
        // Check if already started
        if self.swarm_loop_handle.is_some() {
            println!("⚠️ Node is already started");
            return Ok(());
        }
        
        // Take the swarm_loop and start it in background task
        if let Some(swarm_loop) = self.swarm_loop.take() {
            let swarm_loop_handle = tokio::spawn(async move {
                swarm_loop.run().await
            });
            self.swarm_loop_handle = Some(swarm_loop_handle);
            println!("✅ XNetwork2 node started successfully");
        } else {
            return Err("❌ Cannot start node: swarm_loop is missing".into());
        }
        
        Ok(())
    }

    /// Force shutdown the node (immediate stop via stopper)
    pub async fn force_shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🛑 Force shutting down XNetwork2 node...");
        self.stopper.stop();
        println!("✅ Force shutdown command sent, waiting for task completion...");
        self.wait_for_shutdown().await?;
        println!("✅ XNetwork2 node force shutdown completed");
        Ok(())
    }

    /// Wait for the swarm loop task to complete
    pub async fn wait_for_shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(handle) = self.swarm_loop_handle.take() {
            println!("⏳ Waiting for swarm loop task to complete...");
            match handle.await {
                Ok(result) => {
                    println!("✅ Swarm loop task completed successfully");
                    result
                }
                Err(e) => {
                    println!("❌ Swarm loop task failed: {}", e);
                    Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        } else {
            println!("⚠️ No swarm loop task to wait for");
            Ok(())
        }
    }

    /// Check if the swarm loop task is still running
    pub fn is_running(&self) -> bool {
        if let Some(handle) = &self.swarm_loop_handle {
            !handle.is_finished()
        } else {
            false
        }
    }

    /// Get the status of the swarm loop task
    pub fn get_task_status(&self) -> &'static str {
        if let Some(handle) = &self.swarm_loop_handle {
            if handle.is_finished() {
                "completed"
            } else {
                "running"
            }
        } else {
            "not_started"
        }
    }

    /// Subscribe to NodeEvents from this node
    /// 
    /// Returns a receiver that can be used to listen for events.
    /// Multiple subscribers can be created by calling this method multiple times.
    pub fn subscribe(&self) -> broadcast::Receiver<NodeEvent> {
        self.event_sender.subscribe()
    }

    /// Get the Peer ID of this node
    /// 
    /// Available immediately after node creation, no need to wait for startup.
    pub fn peer_id(&self) -> &libp2p::PeerId {
        &self.peer_id
    }

    /// Get the keypair used by this node
    /// 
    /// Available immediately after node creation, no need to wait for startup.
    pub fn keypair(&self) -> &identity::Keypair {
        &self.keypair
    }
}

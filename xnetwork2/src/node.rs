//! Node creation and management for XNetwork2

use command_swarm::{SwarmLoop, SwarmLoopBuilder, SwarmLoopStopper};
use libp2p::{identity, quic};
use tokio::sync::{broadcast, mpsc};

use crate::node_events::NodeEvent;

use crate::behaviours::{IdentifyHandler, PingHandler, XAuthHandler, XStreamHandler};
use crate::commander::Commander;
use crate::main_behaviour::{
    XNetworkBehaviour, XNetworkBehaviourHandlerDispatcher, XNetworkCommands,
};
use crate::swarm_handler::XNetworkSwarmHandler;

/// XNetwork2 Node
pub struct Node {
    /// Commander for sending commands to the node
    pub command_tx: mpsc::Sender<XNetworkCommands>,
    /// Commander wrapper for convenient command sending with responses
    pub commander: Commander,
    /// Stopper for graceful shutdown
    pub stopper: SwarmLoopStopper,
    /// SwarmLoop before starting (None after start)
    pub swarm_loop:
        Option<SwarmLoop<XNetworkBehaviour, XNetworkBehaviourHandlerDispatcher, XNetworkCommands>>,
    /// Handle to the background swarm loop task (Some after start)
    pub swarm_loop_handle:
        Option<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
    /// Broadcast channel for sending NodeEvents to multiple subscribers
    pub event_sender: broadcast::Sender<NodeEvent>,
    /// Peer ID of this node (available immediately after creation)
    pub peer_id: libp2p::PeerId,
    /// Keypair used by this node (available immediately after creation)
    pub keypair: identity::Keypair,
}

impl Node {
    /// Create a new XNetwork2 node with default configuration
    ///
    /// For backward compatibility, uses AutoApprove policy for inbound streams
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        crate::node_builder::NodeBuilder::new().build().await
    }

    /// Create a new XNetwork2 node with custom configuration using NodeBuilder
    ///
    /// Provides fluent API for configuring node behavior
    pub async fn builder() -> crate::node_builder::NodeBuilder {
        crate::node_builder::NodeBuilder::new()
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
            let swarm_loop_handle = tokio::spawn(async move { swarm_loop.run().await });
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
    pub async fn wait_for_shutdown(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

//! Node creation and management for XNetwork2

use command_swarm::{SwarmLoop, SwarmLoopBuilder, SwarmLoopStopper};
use libp2p::{identity, quic, Multiaddr, PeerId};
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
    /// Uses ManualApprove policy for inbound streams by default
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

    // Convenience methods for common operations

    /// Stop the node (alias for force_shutdown)
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.force_shutdown().await
    }

    // XRoutes convenience methods

    /// Enable identify behaviour
    pub async fn enable_identify(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.enable_identify().await
    }

    /// Disable identify behaviour
    pub async fn disable_identify(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.disable_identify().await
    }

    /// Enable mDNS discovery
    pub async fn enable_mdns(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.enable_mdns().await
    }

    /// Disable mDNS discovery
    pub async fn disable_mdns(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.disable_mdns().await
    }

    /// Enable Kademlia DHT discovery
    pub async fn enable_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.enable_kad().await
    }

    /// Disable Kademlia DHT discovery
    pub async fn disable_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.disable_kad().await
    }

    /// Get current status of XRoutes behaviours
    pub async fn get_xroutes_status(&self) -> Result<crate::behaviours::xroutes::XRoutesStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.get_xroutes_status().await
    }

    /// Bootstrap to a peer for Kademlia DHT
    pub async fn bootstrap_to_peer(
        &self,
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.bootstrap_to_peer(peer_id, addresses).await
    }

    /// Find a peer through Kademlia DHT
    pub async fn find_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.find_peer(peer_id).await
    }

    /// Get closest peers through Kademlia DHT
    pub async fn get_closest_peers(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.get_closest_peers(peer_id).await
    }

    /// Find peer addresses with automatic search and timeout
    pub async fn find_peer_addresses(
        &self,
        peer_id: PeerId,
        timeout: std::time::Duration,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.find_peer_addresses(peer_id, timeout).await
    }

    // mDNS cache methods

    /// Get all peers from mDNS cache
    pub async fn get_mdns_peers(
        &self,
    ) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.get_mdns_peers().await
    }

    /// Find a specific peer in mDNS cache
    pub async fn find_mdns_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<Option<Vec<Multiaddr>>, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.find_mdns_peer(peer_id).await
    }

    /// Get mDNS cache status
    pub async fn get_mdns_cache_status(
        &self,
    ) -> Result<crate::behaviours::xroutes::MdnsCacheStatus, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.get_mdns_cache_status().await
    }

    /// Clear mDNS cache
    pub async fn clear_mdns_cache(
        &self,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        self.commander.clear_mdns_cache().await
    }

    /// Enable mDNS with custom TTL
    pub async fn enable_mdns_with_ttl(
        &self,
        ttl_seconds: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commander.enable_mdns_with_ttl(ttl_seconds).await
    }
}

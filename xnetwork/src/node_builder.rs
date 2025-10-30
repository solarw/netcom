// src/node_builder.rs - Builder pattern for NetworkNode configuration

use libp2p::{identity, PeerId};
use xauth::por::por::ProofOfRepresentation;
use crate::{NetworkNode, XRoutesConfig};
use std::error::Error;

/// Builder for configuring and creating NetworkNode instances
/// 
/// Provides a fluent interface for setting up node parameters
/// and ensures backward compatibility while allowing easy extension.
pub struct NodeBuilder {
    local_key: identity::Keypair,
    por: ProofOfRepresentation,
    config: Option<XRoutesConfig>,
    cmd_queue_size: Option<usize>,
    event_queue_size: Option<usize>,
    enable_mdns: bool,
    server_mode: bool,
}

impl NodeBuilder {
    /// Create a new NodeBuilder with required parameters
    pub fn new(local_key: identity::Keypair, por: ProofOfRepresentation) -> Self {
        Self {
            local_key,
            por,
            config: None,
            cmd_queue_size: None,
            event_queue_size: None,
            enable_mdns: false,
            server_mode: false,
        }
    }

    /// Set XRoutes configuration
    pub fn with_config(mut self, config: XRoutesConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set custom queue sizes for commands and events
    pub fn with_queue_sizes(mut self, cmd_size: usize, event_size: usize) -> Self {
        self.cmd_queue_size = Some(cmd_size);
        self.event_queue_size = Some(event_size);
        self
    }

    /// Enable mDNS discovery
    pub fn enable_mdns(mut self) -> Self {
        self.enable_mdns = true;
        self
    }

    /// Set server mode (enables Kademlia server functionality)
    pub fn server_mode(mut self) -> Self {
        self.server_mode = true;
        self
    }

    /// Build the NetworkNode with configured parameters
    pub async fn build(self) -> Result<
        (
            NetworkNode,
            tokio::sync::mpsc::Sender<crate::NetworkCommand>,
            tokio::sync::mpsc::Receiver<crate::NetworkEvent>,
            PeerId,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        // Use existing constructors with appropriate parameters
        if let Some(config) = self.config {
            // Use new_with_config if XRoutes config is provided
            NetworkNode::new_with_config(
                self.local_key, 
                self.por, 
                Some(config),
                self.cmd_queue_size,
                self.event_queue_size,
            ).await
        } else {
            // Use basic constructor with individual parameters
            NetworkNode::new(self.local_key, self.por, self.enable_mdns, self.server_mode).await
        }
    }

    /// Build the NetworkNode with custom queue sizes
    /// This method will be available once NetworkNode constructors support queue size parameters
    #[allow(dead_code)]
    async fn build_with_queues(self) -> Result<
        (
            NetworkNode,
            tokio::sync::mpsc::Sender<crate::NetworkCommand>,
            tokio::sync::mpsc::Receiver<crate::NetworkEvent>,
            PeerId,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        // This method will be implemented once NetworkNode supports queue size parameters
        // For now, fall back to standard build
        self.build().await
    }
}

/// Convenience methods for common node configurations
impl NodeBuilder {
    /// Create a client node with mDNS discovery enabled
    pub fn client_with_mdns(local_key: identity::Keypair, por: ProofOfRepresentation) -> Self {
        Self::new(local_key, por)
            .enable_mdns()
    }

    /// Create a server node with Kademlia server mode enabled
    pub fn server_with_kademlia(local_key: identity::Keypair, por: ProofOfRepresentation) -> Self {
        Self::new(local_key, por)
            .server_mode()
            .with_config(XRoutesConfig::bootstrap_server())
    }

    /// Create a minimal client node (no discovery protocols)
    pub fn minimal_client(local_key: identity::Keypair, por: ProofOfRepresentation) -> Self {
        Self::new(local_key, por)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_por() -> ProofOfRepresentation {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        
        ProofOfRepresentation::create(
            &keypair,
            peer_id,
            Duration::from_secs(3600),
        ).expect("Failed to create test PoR")
    }

    #[tokio::test]
    async fn test_node_builder_basic() {
        let key = identity::Keypair::generate_ed25519();
        let por = create_test_por();

        let builder = NodeBuilder::new(key, por);
        let result = builder.build().await;

        assert!(result.is_ok(), "Basic node creation should succeed");
    }

    #[tokio::test]
    async fn test_node_builder_with_config() {
        let key = identity::Keypair::generate_ed25519();
        let por = create_test_por();

        let builder = NodeBuilder::new(key, por)
            .with_config(XRoutesConfig::client());
        let result = builder.build().await;

        assert!(result.is_ok(), "Node creation with config should succeed");
    }

    #[tokio::test]
    async fn test_node_builder_convenience_methods() {
        let key = identity::Keypair::generate_ed25519();
        let por = create_test_por();

        // Test client with mDNS
        let client_mdns = NodeBuilder::client_with_mdns(key.clone(), por.clone());
        let result = client_mdns.build().await;
        assert!(result.is_ok(), "Client with mDNS should succeed");

        // Test server with Kademlia
        let server_kad = NodeBuilder::server_with_kademlia(key.clone(), por.clone());
        let result = server_kad.build().await;
        assert!(result.is_ok(), "Server with Kademlia should succeed");

        // Test minimal client
        let minimal = NodeBuilder::minimal_client(key, por);
        let result = minimal.build().await;
        assert!(result.is_ok(), "Minimal client should succeed");
    }
}

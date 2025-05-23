// src/xroutes/commander.rs

use libp2p::{Multiaddr, PeerId};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use super::{XRoutesCommand, XRouteRole};
use super::types::{BootstrapNodeInfo, BootstrapError};

pub struct XRoutesCommander {
    cmd_tx: mpsc::Sender<crate::commands::NetworkCommand>,
}

impl XRoutesCommander {
    pub fn new(cmd_tx: mpsc::Sender<crate::commands::NetworkCommand>) -> Self {
        Self { cmd_tx }
    }

    /// Connect to a bootstrap node and verify it's in server mode
    pub async fn connect_to_bootstrap_node(
        &self,
        addr: Multiaddr,
        timeout_secs: Option<u64>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        let (response_tx, response_rx) = oneshot::channel();

        // Send XRoutes command for bootstrap connection
        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::ConnectToBootstrap {
                    addr,
                    timeout_secs,
                    response: response_tx,
                }
            ))
            .await
            .map_err(|e| BootstrapError::ConnectionFailed(format!("Failed to send command: {}", e)))?;

        response_rx
            .await
            .map_err(|e| BootstrapError::ConnectionFailed(format!("Failed to receive response: {}", e)))?
    }

    /// Bootstrap Kademlia DHT
    pub async fn bootstrap_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::BootstrapKad { response: response_tx }
            ))
            .await?;

        response_rx.await?
    }

    /// Get known peers from Kademlia
    pub async fn get_kad_known_peers(&self) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::GetKadKnownPeers { response: response_tx }
            ))
            .await?;

        Ok(response_rx.await?)
    }

    /// Get addresses for a specific peer
    pub async fn get_peer_addresses(&self, peer_id: PeerId) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::GetPeerAddresses { peer_id, response: response_tx }
            ))
            .await?;

        Ok(response_rx.await?)
    }

    /// Find peer addresses via DHT
    pub async fn find_peer_addresses(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::FindPeerAddresses { peer_id, response: response_tx }
            ))
            .await?;

        response_rx.await?
    }

    /// Set XRoute role
    pub async fn set_role(&self, role: XRouteRole) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::SetRole { role, response: response_tx }
            ))
            .await?;

        response_rx.await??;
        Ok(())
    }

    /// Get current XRoute role
    pub async fn get_role(&self) -> Result<XRouteRole, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::GetRole { response: response_tx }
            ))
            .await?;

        Ok(response_rx.await?)
    }
}
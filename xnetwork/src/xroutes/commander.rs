// src/xroutes/commander.rs

use libp2p::{Multiaddr, PeerId};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use super::{XRoutesCommand, XRouteRole};
use super::discovery::commands::DiscoveryCommand;
use super::discovery::kad::commands::KadCommand;
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
        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::Bootstrap))
            ))
            .await?;

        Ok(())
    }

    /// Get known peers from Kademlia
    pub async fn get_kad_known_peers(&self) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::GetKnownPeers { response: response_tx }))
            ))
            .await?;

        Ok(response_rx.await?)
    }

    /// Get addresses for a specific peer from local tables only
    pub async fn get_peer_addresses(&self, peer_id: PeerId) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::GetPeerAddresses { peer_id, response: response_tx }))
            ))
            .await?;

        Ok(response_rx.await?)
    }

    /// Find peer addresses with advanced timeout control
    /// 
    /// # Arguments
    /// * `peer_id` - The peer to find addresses for
    /// * `timeout_secs` - Timeout behavior:
    ///   - `0` - Check local tables only, no DHT search
    ///   - `>0` - Search with specified timeout in seconds
    ///   - `-1` - Infinite search until explicitly cancelled
    /// 
    /// # Returns
    /// * `Ok(Vec<Multiaddr>)` - Found addresses (may be empty)
    /// * `Err(String)` - Error description (timeout, DHT disabled, etc.)
    pub async fn find_peer_addresses_advanced(
        &self,
        peer_id: PeerId,
        timeout_secs: i32,
    ) -> Result<Vec<Multiaddr>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::FindPeerAddressesAdvanced {
                    peer_id,
                    timeout_secs,
                    response: response_tx,
                }))
            ))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }

    /// Convenience method: Find peer addresses with timeout in seconds
    pub async fn find_peer_addresses_with_timeout(
        &self,
        peer_id: PeerId,
        timeout_secs: u32,
    ) -> Result<Vec<Multiaddr>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::FindPeerAddressesWithTimeout {
                    peer_id,
                    timeout_secs,
                    response: response_tx,
                }))
            ))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }

    /// Convenience method: Find peer addresses from local tables only
    pub async fn find_peer_addresses_local_only(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::FindPeerAddressesLocalOnly {
                    peer_id,
                    response: response_tx,
                }))
            ))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }

    /// Convenience method: Find peer addresses with infinite timeout
    pub async fn find_peer_addresses_infinite(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::FindPeerAddressesInfinite {
                    peer_id,
                    response: response_tx,
                }))
            ))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }

    /// Cancel active search for a specific peer
    pub async fn cancel_peer_search(&self, peer_id: PeerId) -> Result<(), String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::CancelPeerSearch {
                    peer_id,
                    response: response_tx,
                }))
            ))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }

    /// Get information about active searches
    /// Returns list of (peer_id, waiters_count, search_duration)
    pub async fn get_active_searches(&self) -> Result<Vec<(PeerId, usize, Duration)>, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(crate::commands::NetworkCommand::XRoutes(
                XRoutesCommand::Discovery(DiscoveryCommand::Kad(KadCommand::GetActiveSearches {
                    response: response_tx,
                }))
            ))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))
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

    /// Legacy method: Find peer addresses (compatibility)
    pub async fn find_peer_addresses(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Use advanced search with 30 second timeout for compatibility
        match self.find_peer_addresses_advanced(peer_id, 30).await {
            Ok(_addresses) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

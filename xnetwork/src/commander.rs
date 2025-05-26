// src/commander.rs

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use crate::commands::NetworkCommand;
use crate::xroutes::{XRoutesCommand, XRouteRole, XRoutesCommander, BootstrapNodeInfo, BootstrapError};
use xauth::definitions::AuthResult;
use xstream::xstream::XStream;

pub struct Commander {
    cmd_tx: mpsc::Sender<NetworkCommand>,
    pub xroutes: XRoutesCommander,
}

impl Commander {
    pub fn new(cmd_tx: mpsc::Sender<NetworkCommand>) -> Commander {
        let xroutes = XRoutesCommander::new(cmd_tx.clone());
        Commander { 
            cmd_tx,
            xroutes,
        }
    }

    /// Core connection methods
    pub async fn listen_port(
        &self,
        host: Option<String>,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        let host_str = host
            .as_ref()
            .map_or_else(|| "0.0.0.0".to_string(), |h| h.clone());

        self.cmd_tx
            .send(NetworkCommand::OpenListenPort {
                host: host_str,
                port,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send open port command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(addr) => {
                println!("Server is listening on {}", addr);
                return Ok(());
            }
            Err(e) => {
                println!("Failed to listen: {}", e);
                return Err(format!(
                    "Failed to listen on {}:{}: {}",
                    host.as_ref()
                        .map_or_else(|| "0.0.0.0".to_string(), |h| h.clone()),
                    port,
                    e
                )
                .into());
            }
        }
    }

    pub async fn connect(
        &self,
        addr: Multiaddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Connect {
                addr,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send connect command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to connect: {}", e).into()),
        }
    }

    pub async fn disconnect(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Disconnect {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send disconnect command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to disconnect: {}", e).into()),
        }
    }

    /// Get listening addresses
    pub async fn get_listen_addresses(
        &self,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetListenAddresses {
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get listen addresses command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    /// Get connections for a specific peer
    pub async fn get_connections_for_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetConnectionsForPeer {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get connections command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    /// Get all connected peers
    pub async fn get_connected_peers(
        &self,
    ) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetPeersConnected {
                peer_id: PeerId::random(), // Dummy peer_id, will be ignored
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get connected peers command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    /// Core authentication methods
    pub async fn is_peer_authenticated(
        &self,
        peer_id: PeerId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::IsPeerAuthenticated {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send authentication check command: {}", e).into()
            })?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e).into())
    }

    pub async fn submit_por_verification(
        &self,
        connection_id: ConnectionId,
        result: AuthResult,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.cmd_tx
            .send(NetworkCommand::SubmitPorVerification {
                connection_id,
                result,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send PoR verification result: {}", e).into()
            })?;

        Ok(())
    }

    /// Core stream methods
    pub async fn open_stream(&self, peer_id: PeerId) -> Result<XStream, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::OpenStream {
                peer_id: peer_id,
                connection_id: None,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send open stream command: {}", e).into()
            });

        match response_rx.await {
            Ok(result) => result,
            Err(e) => Err(format!("Failed to receive response: {}", e).into()),
        }
    }

    /// Enhanced XRoutes integration - convenience methods that delegate to xroutes commander

    /// Connect to a bootstrap node and verify it's in server mode
    pub async fn connect_to_bootstrap_node(
        &self,
        addr: Multiaddr,
        timeout_secs: Option<u64>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        self.xroutes.connect_to_bootstrap_node(addr, timeout_secs).await
    }

    /// Find peer addresses with advanced timeout control (NEW)
    /// 
    /// # Arguments
    /// * `peer_id` - The peer to find addresses for
    /// * `timeout_secs` - Timeout behavior:
    ///   - `0` - Check local tables only, no DHT search
    ///   - `>0` - Search with specified timeout in seconds
    ///   - `-1` - Infinite search until explicitly cancelled
    pub async fn find_peer_addresses_advanced(
        &self,
        peer_id: PeerId,
        timeout_secs: i32,
    ) -> Result<Vec<Multiaddr>, String> {
        self.xroutes.find_peer_addresses_advanced(peer_id, timeout_secs).await
    }

    /// Convenience: Find peer addresses with timeout in seconds
    pub async fn find_peer_addresses_with_timeout(
        &self,
        peer_id: PeerId,
        timeout_secs: u32,
    ) -> Result<Vec<Multiaddr>, String> {
        self.xroutes.find_peer_addresses_with_timeout(peer_id, timeout_secs).await
    }

    /// Convenience: Find peer addresses from local tables only
    pub async fn find_peer_addresses_local_only(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, String> {
        self.xroutes.find_peer_addresses_local_only(peer_id).await
    }

    /// Convenience: Find peer addresses with infinite timeout
    pub async fn find_peer_addresses_infinite(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, String> {
        self.xroutes.find_peer_addresses_infinite(peer_id).await
    }

    /// Cancel active search for a specific peer
    pub async fn cancel_peer_search(&self, peer_id: PeerId) -> Result<(), String> {
        self.xroutes.cancel_peer_search(peer_id).await
    }

    /// Get information about active searches
    pub async fn get_active_searches(&self) -> Result<Vec<(PeerId, usize, Duration)>, String> {
        self.xroutes.get_active_searches().await
    }

    /// Legacy method - kept for compatibility
    pub async fn find_peer_addresses(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.xroutes.find_peer_addresses(peer_id).await
    }

    /// Bootstrap Kademlia DHT
    pub async fn bootstrap_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.xroutes.bootstrap_kad().await
    }

    /// Get known peers from Kademlia
    pub async fn get_kad_known_peers(&self) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        self.xroutes.get_kad_known_peers().await
    }

    /// Get addresses for a specific peer from local tables
    pub async fn get_peer_addresses(&self, peer_id: PeerId) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        self.xroutes.get_peer_addresses(peer_id).await
    }

    /// Set XRoute role
    pub async fn set_xroute_role(&self, role: XRouteRole) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.xroutes.set_role(role).await
    }

    /// Get current XRoute role
    pub async fn get_xroute_role(&self) -> Result<XRouteRole, Box<dyn std::error::Error + Send + Sync>> {
        self.xroutes.get_role().await
    }

    /// Shutdown the network node
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.cmd_tx
            .send(NetworkCommand::Shutdown)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send shutdown command: {}", e).into()
            })?;

        Ok(())
    }
}
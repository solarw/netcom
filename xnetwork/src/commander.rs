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

    /// New bootstrap connection method - delegate to xroutes
    pub async fn connect_to_bootstrap_node(
        &self,
        addr: Multiaddr,
        timeout_secs: Option<u64>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        self.xroutes.connect_to_bootstrap_node(addr, timeout_secs).await
    }
}
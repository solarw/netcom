use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use super::commands::NetworkCommand;
use super::xauth::definitions::AuthResult;
use super::xstream::manager::XStream;

pub struct Commander {
    cmd_tx: mpsc::Sender<NetworkCommand>,
}

impl Commander {
    pub fn new(cmd_tx: mpsc::Sender<NetworkCommand>) -> Commander {
        Commander { cmd_tx }
    }

    pub async fn listen_port(
        &self,
        host: Option<String>,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        // Use provided host or default to 0.0.0.0 (all interfaces)
        // Use as_ref() to borrow instead of move
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
                // Using as_ref() again to avoid moving host
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

    pub async fn get_kad_known_peers(
        &self,
    ) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetKadKnownPeers {
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get kad peers command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    // Get known addresses for a peer from the Kademlia DHT
    pub async fn get_peer_addresses(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetPeerAddresses {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get peer addresses command: {}", e).into()
            })?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e).into())
    }

    // Initiate a network search for a peer's addresses
    pub async fn find_peer_addresses(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::FindPeerAddresses {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send find peer addresses command: {}", e).into()
            })?;

        match response_rx.await {
            Ok(result) => result,
            Err(e) => Err(format!("Failed to receive response: {}", e).into()),
        }
    }

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

    // New method to submit PoR verification result
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
                format!("Failed to send find peer addresses command: {}", e).into()
            });

        match response_rx.await {
            Ok(result) => result,
            Err(e) => Err(format!("Failed to receive response: {}", e).into()),
        }
    }

    // Enhanced method for finding peer addresses through Kademlia with proper bootstrap and retries
    pub async fn search_peer_addresses(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        // First, broadcast ourselves to the network
        println!("Broadcasting our presence to the network...");

        // Make sure Kademlia is enabled
        match self.cmd_tx.send(NetworkCommand::EnableKad).await {
            Ok(_) => println!("Kademlia enabled"),
            Err(e) => println!("Failed to enable Kademlia: {}", e),
        }

        // Bootstrap Kademlia to announce our presence to the network
        let (response_tx, response_rx) = oneshot::channel();
        match self
            .cmd_tx
            .send(NetworkCommand::BootstrapKad {
                response: response_tx,
            })
            .await
        {
            Ok(_) => match response_rx.await {
                Ok(result) => match result {
                    Ok(_) => println!("Kademlia bootstrap initiated"),
                    Err(e) => println!("Kademlia bootstrap error: {}", e),
                },
                Err(e) => println!("Bootstrap response channel error: {}", e),
            },
            Err(e) => println!("Failed to send bootstrap command: {}", e),
        }

        // Start looking for the peer
        println!("Initiating search for peer: {}", peer_id);

        // Start an explicit DHT search
        self.find_peer_addresses(peer_id).await?;

        // Wait a bit longer for the search to propagate (3 seconds instead of 1)
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Try multiple times with increasing delay
        for attempt in 1..=5 {
            println!("Attempt {} to find peer {} via Kademlia", attempt, peer_id);

            // Get the latest addresses
            let addrs = self.get_peer_addresses(peer_id).await?;

            if addrs.is_empty() {
                println!(
                    "No addresses found for peer {} on attempt {}",
                    peer_id, attempt
                );
                // Retry the search before waiting
                self.find_peer_addresses(peer_id).await?;
                // Use exponential backoff for waiting
                tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
                continue;
            }

            // Return the addresses we found
            println!("Found {} addresses for peer {}", addrs.len(), peer_id);
            return Ok(addrs);
        }

        Err("Failed to find peer addresses via Kademlia after multiple attempts".into())
    }

    pub async fn bootstrap_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::BootstrapKad {
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send bootstrap Kad command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(_) => {
                println!("Kademlia bootstrap initiated successfully");
                Ok(())
            }
            Err(e) => Err(format!("Failed to bootstrap Kademlia: {}", e).into()),
        }
    }
}

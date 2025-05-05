use libp2p::{Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use super::commands::NetworkCommand;

pub struct Commander {
    cmd_tx: mpsc::Sender<NetworkCommand>,
}

impl Commander {
    pub fn new(cmd_tx: mpsc::Sender<NetworkCommand>) -> Commander {
        Commander { cmd_tx }
    }

    pub async fn listen_port(
        &self,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::OpenListenPort {
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
                return Err(format!("Failed to listen on port {}: {}", port, e).into());
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


    pub async fn get_kad_known_peers(&self) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
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
        
        response_rx.await.map_err(|e| format!("Failed to receive response: {}", e).into())
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
}

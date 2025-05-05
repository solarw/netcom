use libp2p::Multiaddr;
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
}

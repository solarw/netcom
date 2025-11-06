//! Commander for sending commands to XNetwork2 node

use tokio::sync::{mpsc, oneshot};
use libp2p::{Multiaddr, PeerId};

use crate::main_behaviour::XNetworkCommands;
use crate::swarm_commands::{SwarmLevelCommand, NetworkState};
use crate::behaviours::XAuthCommand;

/// Commander for XNetwork2 node
#[derive(Clone)]
pub struct Commander {
    sender: mpsc::Sender<XNetworkCommands>,
    stopper: command_swarm::SwarmLoopStopper,
}

impl Commander {
    /// Create a new commander
    pub fn new(sender: mpsc::Sender<XNetworkCommands>, stopper: command_swarm::SwarmLoopStopper) -> Self {
        Self { sender, stopper }
    }

    /// Send a command to the node
    pub async fn send(&self, command: XNetworkCommands) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sender.send(command).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Dial a peer
    pub async fn dial(&self, peer_id: PeerId, addr: Multiaddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::Dial {
            peer_id,
            addr,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Listen on an address
    pub async fn listen_on(&self, addr: Multiaddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ListenOn {
            addr,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Disconnect from a peer
    pub async fn disconnect(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::Disconnect {
            peer_id,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get network state
    pub async fn get_network_state(&self) -> Result<NetworkState, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::GetNetworkState {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Shutdown the node
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::Shutdown {
            stopper: self.stopper.clone(),
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Send echo command and get response
    pub async fn echo(&self, message: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::Echo {
            message,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Start authentication with a peer
    pub async fn start_auth(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let command = XNetworkCommands::xauth(XAuthCommand::StartAuth { peer_id });
        self.send(command).await
    }

    /// Approve authentication request
    pub async fn approve_auth(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let command = XNetworkCommands::xauth(XAuthCommand::ApproveAuth { peer_id });
        self.send(command).await
    }

    /// Reject authentication request
    pub async fn reject_auth(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let command = XNetworkCommands::xauth(XAuthCommand::RejectAuth { peer_id });
        self.send(command).await
    }

    /// Submit PoR verification result
    pub async fn submit_por_verification(
        &self, 
        peer_id: PeerId, 
        approved: bool
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let command = XNetworkCommands::xauth(XAuthCommand::SubmitPorVerification {
            peer_id,
            approved,
        });
        self.send(command).await
    }
}

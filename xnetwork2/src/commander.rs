//! Commander for sending commands to XNetwork2 node

use libp2p::{Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::behaviours::{XAuthCommand, XStreamCommand};
use crate::main_behaviour::XNetworkCommands;
use crate::swarm_commands::{NetworkState, SwarmLevelCommand};
use xstream::xstream::XStream;

/// Commander for XNetwork2 node
#[derive(Clone)]
pub struct Commander {
    sender: mpsc::Sender<XNetworkCommands>,
    stopper: command_swarm::SwarmLoopStopper,
}

impl Commander {
    /// Create a new commander
    pub fn new(
        sender: mpsc::Sender<XNetworkCommands>,
        stopper: command_swarm::SwarmLoopStopper,
    ) -> Self {
        Self { sender, stopper }
    }

    /// Send a command to the node
    pub async fn send(
        &self,
        command: XNetworkCommands,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sender
            .send(command)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Dial a peer
    pub async fn dial(
        &self,
        peer_id: PeerId,
        addr: Multiaddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    pub async fn listen_on(
        &self,
        addr: Multiaddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ListenOn {
            addr,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get network state
    pub async fn get_network_state(
        &self,
    ) -> Result<NetworkState, Box<dyn std::error::Error + Send + Sync>> {
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
    pub async fn echo(
        &self,
        message: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::Echo {
            message,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Submit PoR verification result
    pub async fn submit_por_verification(
        &self,
        peer_id: PeerId,
        approved: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let command =
            XNetworkCommands::xauth(XAuthCommand::SubmitPorVerification { peer_id, approved });
        self.send(command).await
    }

    /// Open XStream to a peer
    pub async fn open_xstream(
        &self,
        peer_id: PeerId,
    ) -> Result<XStream, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Add authentication check here once we have access to authenticated peers
        // For now, we'll allow opening XStream without authentication check
        // This should be fixed when we integrate with the swarm handler's authenticated_peers

        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xstream(XStreamCommand::OpenStream {
            peer_id,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?.map_err(|e| {
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
                as Box<dyn std::error::Error + Send + Sync>
        })
    }

    // XRoutes commands

    /// Enable identify behaviour
    pub async fn enable_identify(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::EnableIdentify {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Disable identify behaviour
    pub async fn disable_identify(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::DisableIdentify {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Enable mDNS discovery
    pub async fn enable_mdns(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::EnableMdns {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Disable mDNS discovery
    pub async fn disable_mdns(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::DisableMdns {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Enable Kademlia DHT discovery
    pub async fn enable_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::EnableKad {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Disable Kademlia DHT discovery
    pub async fn disable_kad(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::DisableKad {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get current status of XRoutes behaviours
    pub async fn get_xroutes_status(&self) -> Result<crate::behaviours::xroutes::XRoutesStatus, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::GetStatus {
            response: response_tx,
        });
        self.send(command).await?;
        Ok(response_rx.await?)
    }

    /// Bootstrap to a peer for Kademlia DHT
    pub async fn bootstrap_to_peer(
        &self,
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::BootstrapToPeer {
            peer_id,
            addresses,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Find a peer through Kademlia DHT
    pub async fn find_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::FindPeer {
            peer_id,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get closest peers through Kademlia DHT
    pub async fn get_closest_peers(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::GetClosestPeers {
            peer_id,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Find peer addresses with automatic search and timeout
    pub async fn find_peer_addresses(
        &self,
        peer_id: PeerId,
        timeout: std::time::Duration,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::FindPeerAddresses {
            peer_id,
            timeout,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }
}

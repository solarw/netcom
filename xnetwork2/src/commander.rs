//! Commander for sending commands to XNetwork2 node

use libp2p::core::transport::ListenerId;
use libp2p::{Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::behaviours::{XAuthCommand, XStreamCommand};
use crate::conntracker::commands::ConntrackerCommand;
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
    ) -> Result<ListenerId, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ListenOn {
            addr,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Listen on an address and wait for first listen address event
    pub async fn listen_and_wait(
        &self,
        addr: Multiaddr,
        timeout: std::time::Duration,
    ) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ListenAndWait {
            addr,
            timeout,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Dial a peer and wait for connection established
    pub async fn dial_and_wait(
        &self,
        peer_id: PeerId,
        addr: Multiaddr,
        timeout: std::time::Duration,
    ) -> Result<libp2p::swarm::ConnectionId, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::DialAndWait {
            peer_id,
            addr,
            timeout,
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


    /// Start authentication for specific connection
    pub async fn start_auth_for_connection(
        &self,
        connection_id: libp2p::swarm::ConnectionId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::StartAuthForConnection {
            connection_id,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
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

    // mDNS cache commands

    /// Get all peers from mDNS cache
    pub async fn get_mdns_peers(
        &self,
    ) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::GetMdnsPeers {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Find a specific peer in mDNS cache
    pub async fn find_mdns_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<Option<Vec<Multiaddr>>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::FindMdnsPeer {
            peer_id,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get mDNS cache status
    pub async fn get_mdns_cache_status(
        &self,
    ) -> Result<crate::behaviours::xroutes::MdnsCacheStatus, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::GetMdnsCacheStatus {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Clear mDNS cache
    pub async fn clear_mdns_cache(
        &self,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::ClearMdnsCache {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Enable mDNS with custom TTL
    pub async fn enable_mdns_with_ttl(
        &self,
        ttl_seconds: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::EnableMdnsWithTtl {
            ttl_seconds,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Enable relay server
    pub async fn enable_relay_server(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::EnableRelayServer {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Add a peer as AutoNAT server
    pub async fn add_autonat_server(
        &self,
        peer_id: PeerId,
        address: Option<Multiaddr>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::AddAutonatServer {
            peer_id,
            address,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Set Kademlia mode (client, server, auto)
    pub async fn set_kad_mode(
        &self,
        mode: crate::behaviours::xroutes::types::KadMode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::SetKadMode {
            mode,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get current Kademlia mode
    pub async fn get_kad_mode(
        &self,
    ) -> Result<crate::behaviours::xroutes::types::KadMode, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::xroutes(crate::behaviours::xroutes::XRoutesCommand::GetKadMode {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    // ConnectionTracker commands

    /// Get all connections
    pub async fn get_connections(
        &self,
    ) -> Result<Vec<crate::conntracker::ConnectionInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ConnectionTracker {
            command: ConntrackerCommand::GetConnections {
                response: response_tx,
            },
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get connections for a specific peer
    pub async fn get_peer_connections(
        &self,
        peer_id: PeerId,
    ) -> Result<crate::conntracker::PeerConnections, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ConnectionTracker {
            command: ConntrackerCommand::GetPeerConnections {
                peer_id,
                response: response_tx,
            },
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get information about a specific connection
    pub async fn get_connection(
        &self,
        connection_id: command_swarm::ConnectionId,
    ) -> Result<crate::conntracker::ConnectionInfo, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ConnectionTracker {
            command: ConntrackerCommand::GetConnection {
                connection_id,
                response: response_tx,
            },
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get all connected peers
    pub async fn get_connected_peers(
        &self,
    ) -> Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ConnectionTracker {
            command: ConntrackerCommand::GetConnectedPeers {
                response: response_tx,
            },
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get connection statistics
    pub async fn get_connection_stats(
        &self,
    ) -> Result<crate::conntracker::ConnectionStats, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ConnectionTracker {
            command: ConntrackerCommand::GetConnectionStats {
                response: response_tx,
            },
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get listen addresses
    pub async fn get_listen_addresses(
        &self,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::ConnectionTracker {
            command: ConntrackerCommand::GetListenAddresses {
                response: response_tx,
            },
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get external addresses from ConnectionTracker
    pub async fn get_external_addresses(
        &self,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::GetExternalAddresses {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Add external address to swarm
    pub async fn add_external_address(
        &self,
        address: Multiaddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::AddExternalAddress {
            address,
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }

    /// Get all external addresses from swarm
    pub async fn get_swarm_external_addresses(
        &self,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();
        let command = XNetworkCommands::SwarmLevel(SwarmLevelCommand::GetExternalAddresses {
            response: response_tx,
        });
        self.send(command).await?;
        response_rx.await?
    }
}

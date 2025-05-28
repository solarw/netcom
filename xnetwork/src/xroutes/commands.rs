// src/xroutes/commands.rs

use libp2p::Multiaddr;
use tokio::sync::oneshot;

use super::discovery::commands::DiscoveryCommand;
use super::connectivity::commands::ConnectivityCommand;
use super::discovery::kad::commands::{BootstrapError, BootstrapNodeInfo};
use super::xroute::XRouteRole;

#[derive(Debug)]
pub enum XRoutesCommand {
    /// Discovery commands (includes mDNS and Kademlia)
    Discovery(DiscoveryCommand),

    /// Connectivity commands (includes relay)
    Connectivity(ConnectivityCommand),

    /// XRoute role management
    SetRole {
        role: XRouteRole,
        response: oneshot::Sender<Result<(), String>>,
    },
    GetRole {
        response: oneshot::Sender<XRouteRole>,
    },

    /// Bootstrap connection (high-level command that uses discovery internally)
    ConnectToBootstrap {
        addr: Multiaddr,
        timeout_secs: Option<u64>,
        response: oneshot::Sender<Result<BootstrapNodeInfo, BootstrapError>>,
    },
}

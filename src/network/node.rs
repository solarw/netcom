use libp2p::futures::StreamExt;

use libp2p::{identify, identity, kad, mdns, Multiaddr, PeerId, Swarm};
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::error;

use std::collections::HashMap;
use std::error::Error;

use tracing::{info, warn};

use super::{
    behaviour::{make_behaviour, NodeBehaviour, NodeBehaviourEvent},
    commands::NetworkCommand,
    events::NetworkEvent,
};
pub struct NetworkNode {
    cmd_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
    swarm: Swarm<NodeBehaviour>,
    mdns_enabled: bool,
    connected_peers: HashMap<PeerId, Vec<Multiaddr>>,
    local_peer_id: PeerId,
}

impl NetworkNode {
    // Create a new NetworkNode with all required protocols
    pub async fn new(
        local_key: identity::Keypair,
    ) -> Result<
        (
            Self,
            mpsc::Sender<NetworkCommand>,
            mpsc::Receiver<NetworkEvent>,
            PeerId,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        // Create a keypair for authentication
        let local_peer_id = PeerId::from(local_key.public());

        // Create a SwarmBuilder with QUIC transport
        let swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| make_behaviour(key))?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(std::time::Duration::from_secs(60))
            })
            .build();

        // Set up communication channels
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);

        Ok((
            Self {
                cmd_rx,
                event_tx,
                swarm,
                mdns_enabled: true,
                connected_peers: HashMap::new(),
                local_peer_id,
            },
            cmd_tx,
            event_rx,
            local_peer_id,
        ))
    }

    // Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    // Get the current listening addresses
    pub fn listening_addresses(&self) -> Vec<Multiaddr> {
        self.swarm.listeners().cloned().collect()
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    if let NetworkCommand::Shutdown = cmd {
                        info!("Shutting down network node");
                        break;
                    }

                    self.handle_command(cmd).await;
                }
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
            }
        }
    }

    // Handle commands sent to the network node
    async fn handle_command(&mut self, cmd: NetworkCommand) {
        match cmd {
            NetworkCommand::EnableMdns => {
                self.mdns_enabled = true;
                info!("mDNS discovery enabled");
            }
            NetworkCommand::DisableMdns => {
                self.mdns_enabled = false;
                info!("mDNS discovery disabled");
            }

            NetworkCommand::OpenListenPort { port, response } => {
                let addr = Multiaddr::from_str(&format!("/ip4/0.0.0.0/udp/{}/quic-v1", port))
                    .expect("Invalid multiaddr");

                match self.swarm.listen_on(addr.clone()) {
                    Ok(_) => {
                        for i in self.listening_addresses() {
                            println!("listenting on {}", i);
                        }

                        let _ = response.send(Ok(addr));
                    }
                    Err(err) => {
                        error!("Failed to listen on {addr}: {err}");
                        let _ = response.send(Err(Box::new(err)));
                    }
                }
            }
            NetworkCommand::Connect { addr, response } => match self.swarm.dial(addr.clone()) {
                Ok(_) => {
                    info!("Dialing {addr}");
                    let _ = response.send(Ok(()));
                }
                Err(err) => {
                    error!("Failed to dial {addr}: {err}");
                    let _ = response.send(Err(Box::new(err)));
                }
            },
            NetworkCommand::Disconnect { peer_id, response } => {
                if self.connected_peers.contains_key(&peer_id) {
                    if let Err(err) = self.swarm.disconnect_peer_id(peer_id) {
                        error!("Failed to disconnect from {peer_id}: {err:?}");
                        let _ = response.send(Err(format!("Failed to disconnect: {err:?}").into()));
                    } else {
                        info!("Disconnected from {peer_id}");
                        self.connected_peers.remove(&peer_id);
                        let _ = response.send(Ok(()));
                    }
                } else {
                    let _ = response.send(Err(format!("Not connected to {peer_id}").into()));
                }
            }
            NetworkCommand::GetConnectionsForPeer { peer_id, response } => {
                let connections = self
                    .connected_peers
                    .get(&peer_id)
                    .unwrap_or(&Vec::new())
                    .to_vec();

                let _ = response.send(connections);
            }
            NetworkCommand::Shutdown => {
                // Handled in the run loop
            }
            _ => {}
        }
    }

    // Handle events emitted by the swarm
    async fn handle_swarm_event(&mut self, event: libp2p::swarm::SwarmEvent<NodeBehaviourEvent>) {
        match event {
            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {address}");
            }
            libp2p::swarm::SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                let addr = endpoint.get_remote_address().clone();
                info!("Connected to {peer_id} at {addr}");

                self.connected_peers
                    .entry(peer_id)
                    .or_insert_with(Vec::new)
                    .push(addr.clone());

                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerConnected { peer_id })
                    .await;
            }
            libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                if let Some(_) = self.connected_peers.remove(&peer_id) {
                    info!("Disconnected from {peer_id}, cause: {cause:?}");
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDisconnected { peer_id })
                        .await;
                }
            }
            libp2p::swarm::SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("Failed to connect to {:?}: {error}", peer_id);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionError {
                        peer_id,
                        error: error.to_string(),
                    })
                    .await;
            }
            libp2p::swarm::SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                ..
            } => {
                warn!("Failed incoming connection from {send_back_addr} to {local_addr}: {error}");
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionError {
                        peer_id: None,
                        error: error.to_string(),
                    })
                    .await;
            }
            libp2p::swarm::SwarmEvent::Behaviour(event) => {
                self.handle_behaviour_event(event).await;
            }
            _ => {}
        }
    }

    // Handle events from the network behaviour
    async fn handle_behaviour_event(&mut self, event: NodeBehaviourEvent) {
        match event {
            NodeBehaviourEvent::Mdns(mdns::Event::Discovered(peers)) => {
                if self.mdns_enabled {
                    for (peer_id, addr) in peers {
                        info!("mDNS discovered peer: {peer_id} at {addr}");
                        self.swarm
                            .behaviour_mut()
                            .kad
                            .add_address(&peer_id, addr.clone());
                    }
                }
            }
            NodeBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                info!("Identified peer {peer_id}: {info:?}");

                // Add peer's listening addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                }

                // Check if peer supports relay protocol
                if info
                    .protocols
                    .iter()
                    .any(|p| p.as_ref().starts_with("/libp2p/circuit/relay"))
                {
                    info!("Peer {peer_id} is a relay");
                }
            }
            NodeBehaviourEvent::Kad(kad::Event::OutboundQueryProgressed { result, .. }) => {
                match result {
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                        key,
                        providers,
                        ..
                    })) => {
                        for peer in providers {
                            info!("Found provider for {key:?}: {peer}");
                        }
                    }
                    kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk {
                        key,
                        peers,
                    })) => {
                        if !peers.is_empty() {
                            info!("Found closest peers for {key:?}: {peers:?}");
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

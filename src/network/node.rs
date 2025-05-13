use libp2p::futures::StreamExt;

use libp2p::{identify, identity, kad, mdns, Multiaddr, PeerId, Swarm};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::error;

use std::collections::{HashMap, HashSet};
use std::error::Error;

use tracing::{info, warn};

use crate::network::xstream::events::XStreamEvent;

use super::xauth::events::PorAuthEvent;
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
    // New field for tracking authenticated peers
    authenticated_peers: HashSet<PeerId>,
}

impl NetworkNode {
    // Create a new NetworkNode with all required protocols
    pub async fn new(
        local_key: identity::Keypair,
        por: super::xauth::por::por::ProofOfRepresentation,
        enable_mdns: bool,
        kad_server_mode: bool, // Add parameter
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
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| make_behaviour(key, por, enable_mdns, kad_server_mode))?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(std::time::Duration::from_secs(60000))
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
                authenticated_peers: HashSet::new(),
            },
            cmd_tx,
            event_rx,
            local_peer_id,
        ))
    }

    pub fn is_peer_authenticated(&self, peer_id: &PeerId) -> bool {
        self.authenticated_peers.contains(peer_id)
    }

    async fn find_peer_addresses(&mut self, peer_id: PeerId) {
        // Инициируем поиск узла в сети
        self.swarm.behaviour_mut().kad.get_closest_peers(peer_id);
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
            println!("loop");
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
            NetworkCommand::OpenStream {
                peer_id,
                connection_id: _,
                response,
            } => {
                self.swarm
                    .behaviour_mut()
                    .xstream
                    .open_stream(peer_id, response)
                    .await;
            }

            NetworkCommand::SubmitPorVerification {
                connection_id,
                result,
            } => {
                info!("Submitting PoR verification result for connection {connection_id}");

                // Submit the verification result to the por_auth behaviour
                match self
                    .swarm
                    .behaviour_mut()
                    .por_auth
                    .submit_por_verification_result(connection_id, result)
                {
                    Ok(_) => info!("✅ Successfully submitted verification result"),
                    Err(e) => warn!("❌ Failed to submit verification result: {}", e),
                }
            }
            NetworkCommand::IsPeerAuthenticated { peer_id, response } => {
                let is_authenticated = self.authenticated_peers.contains(&peer_id)
                    || self
                        .swarm
                        .behaviour()
                        .por_auth
                        .is_peer_authenticated(&peer_id);

                let _ = response.send(is_authenticated);
            }

            NetworkCommand::GetPeerAddresses { peer_id, response } => {
                // Use Kademlia's routing table to get addresses
                let mut addresses = Vec::new();

                // Get the Kademlia behavior from the swarm
                // Iterate through all the k-buckets
                for bucket in self.swarm.behaviour_mut().kad.kbuckets() {
                    // Iterate through all entries in the bucket
                    for entry in bucket.iter() {
                        // Check if this entry matches the peer we're looking for
                        if entry.node.key.preimage() == &peer_id {
                            // Add the addresses to our result
                            addresses = entry.node.value.clone().into_vec();
                            break;
                        }
                    }

                    // If we've found addresses, no need to check more buckets
                    if !addresses.is_empty() {
                        break;
                    }
                }

                let _ = response.send(addresses);
            }

            NetworkCommand::FindPeerAddresses { peer_id, response } => {
                // Initiate a Kademlia search for the peer
                self.swarm.behaviour_mut().kad.get_closest_peers(peer_id);
                // For now, just acknowledge that the search was started
                let _ = response.send(Ok(()));
            }

            NetworkCommand::GetKadKnownPeers { response } => {
                let mut peers = Vec::new();
                // Access the routing table in a different way
                // NOT IMPLEMENTED

                for kbucketref in self.swarm.behaviour_mut().kad.kbuckets() {
                    for i in kbucketref.iter() {
                        let addresses = i.node.value.clone().into_vec();
                        let peer_id = i.node.key.into_preimage();
                        if !addresses.is_empty() {
                            peers.push((peer_id, addresses));
                        }
                    }
                }

                let _ = response.send(peers);
            }
            NetworkCommand::EnableMdns => {
                self.mdns_enabled = true;
                info!("mDNS discovery enabled");
            }
            NetworkCommand::DisableMdns => {
                self.mdns_enabled = false;
                info!("mDNS discovery disabled");
            }

            NetworkCommand::OpenListenPort {
                host,
                port,
                response,
            } => {
                let addr = Multiaddr::from_str(&format!("/ip4/{}/udp/{}/quic-v1", host, port))
                    .expect("Invalid multiaddr");

                match self.swarm.listen_on(addr.clone()) {
                    Ok(_) => {
                        for i in self.listening_addresses() {
                            println!("Listening on {}", i);
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
            NetworkCommand::EnableKad => {
                info!("Kademlia discovery enabled");
                // Try to bootstrap immediately
                if let Err(e) = self.swarm.behaviour_mut().kad.bootstrap() {
                    warn!("Failed to bootstrap Kademlia: {}", e);
                }
            }

            NetworkCommand::BootstrapKad { response } => {
                info!("Bootstrapping Kademlia DHT");
                let result = match self.swarm.behaviour_mut().kad.bootstrap() {
                    Ok(_) => {
                        info!("Kademlia bootstrap started successfully");
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Failed to bootstrap Kademlia: {}", e);
                        Err(Box::new(e) as Box<dyn Error + Send + Sync>)
                    }
                };
                let _ = response.send(result);
            }

            NetworkCommand::Shutdown => {
                // Handled in the run loop
            }

            _ => {}
        }
    }
    async fn handle_swarm_event(&mut self, event: libp2p::swarm::SwarmEvent<NodeBehaviourEvent>) {
        match event {
            libp2p::swarm::SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                connection_id,
                ..
            } => {
                let addr = endpoint.get_remote_address().clone();

                // Track this connection
                self.connected_peers
                    .entry(peer_id)
                    .or_insert_with(Vec::new)
                    .push(addr.clone());

                info!("Connected to {peer_id} at {addr}");

                // Add the peer to Kademlia when connection is established
                // This improves bootstrapping when using --find-peer
                self.swarm
                    .behaviour_mut()
                    .kad
                    .add_address(&peer_id, addr.clone());

                // Explicitly bootstrap Kademlia on new connections
                if let Err(e) = self.swarm.behaviour_mut().kad.bootstrap() {
                    warn!("Failed to bootstrap Kademlia on new connection: {}", e);
                }

                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionOpened {
                        peer_id,
                        addr: addr.clone(),
                        connection_id,
                        protocols: Vec::new(),
                    })
                    .await;

                // Only emit PeerConnected event if this is the first connection
                if num_established.get() == 1 {
                    info!("First connection to peer {peer_id} established");
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerConnected { peer_id })
                        .await;
                }
            }
            libp2p::swarm::SwarmEvent::ConnectionClosed {
                peer_id,
                cause,
                endpoint,
                connection_id,
                num_established, // Remaining established connections to this peer
                ..
            } => {
                let addr = endpoint.get_remote_address().clone();

                // Update our connection tracking - remove this specific connection
                if let Some(connections) = self.connected_peers.get_mut(&peer_id) {
                    connections.retain(|a| a != &addr);

                    // If we have no more connections, remove the peer entry
                    if connections.is_empty() {
                        self.connected_peers.remove(&peer_id);
                    }
                }

                // Always emit ConnectionClosed event
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ConnectionClosed {
                        peer_id,
                        addr,
                        connection_id,
                    })
                    .await;


                // If no connections remain, emit PeerDisconnected
                if num_established == 0 {
                    info!("Disconnected from {peer_id}, cause: {cause:?}");
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDisconnected { peer_id })
                        .await;
                }
            }

            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                // Создаем полный адрес с Peer ID
                let mut full_addr = address.clone();
                full_addr.push(libp2p::multiaddr::Protocol::P2p(self.local_peer_id.into()));

                info!("Listening on {} (with PeerId: {})", address, full_addr);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::ListeningOnAddress {
                        addr: address,
                        full_addr: Some(full_addr),
                    })
                    .await;
            }

            libp2p::swarm::SwarmEvent::ExpiredListenAddr { address, .. } => {
                info!("Stopped listening on {address}");
                let _ = self
                    .event_tx
                    .send(NetworkEvent::StopListeningOnAddress { addr: address })
                    .await;
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

            rest => {
                println!("!!! Rest sawrm {:?} ", rest);
            }
        }
    }

    // Handle events from the network behaviour
    async fn handle_behaviour_event(&mut self, event: NodeBehaviourEvent) {
        match event {
            NodeBehaviourEvent::Xstream(event) => match event {
                XStreamEvent::IncomingStream { stream } => {
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::IncomingStream {
                            stream: Arc::new(stream),
                        })
                        .await;
                }
                rest => {
                    println!("XSTREAM EVENT {:?}", rest);
                }
            },

            NodeBehaviourEvent::Mdns(mdns::Event::Discovered(peers)) => {
                if self.mdns_enabled {
                    for (peer_id, addr) in peers {
                        info!("mDNS discovered peer: {peer_id} at {addr}");
                        // Log when we add an address to Kademlia
                        self.swarm
                            .behaviour_mut()
                            .kad
                            .add_address(&peer_id, addr.clone());

                        info!("📚 Added address to Kademlia: {peer_id} at {addr}");

                        // Optionally send an event about the Kademlia address addition
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::KadAddressAdded {
                                peer_id,
                                addr: addr.clone(),
                            })
                            .await;
                    }
                }
            }

            NodeBehaviourEvent::Kad(ref kad_event) => {
                match kad_event {
                    kad::Event::RoutingUpdated {
                        peer, addresses, ..
                    } => {
                        info!("📔 Kademlia routing updated for peer: {peer}");
                        for addr in addresses.iter() {
                            info!("📕 Known address: {addr}");
                        }

                        // Отправляем событие об обновлении маршрута
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::KadRoutingUpdated {
                                peer_id: *peer,
                                addresses: addresses.iter().cloned().collect(),
                            })
                            .await;
                    }

                    kad::Event::PendingRoutablePeer { peer, .. } => {
                        info!("🔍 Kademlia looking for addresses of peer: {peer}");

                        // Proactively trigger a search for the peer
                        self.swarm.behaviour_mut().kad.get_closest_peers(*peer);
                    }

                    kad::Event::OutboundQueryProgressed { result, .. } => match result {
                        kad::QueryResult::GetProviders(Ok(
                            kad::GetProvidersOk::FoundProviders { key, providers, .. },
                        )) => {
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

                        some => {
                            info!("KAD OUTBOUND{result:?}");
                        }
                    },

                    kad::Event::InboundRequest { request, .. } => {
                        info!(
                            "📥 Received GET_CLOSEST_PEERS request for key: {:?} from peer",
                            request
                        );
                    }

                    // Другие обработчики...
                    some => {
                        info!(" OTHER KAD KAD {some:?}");
                    }
                }
            }

            NodeBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                info!("Identified peer {peer_id}: {info:?}");

                // Add peer's listening addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm
                        .behaviour_mut()
                        .kad
                        .add_address(&peer_id, addr.clone());
                    info!("Address added {peer_id} {addr}");
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
            // Handle PorAuth events
            NodeBehaviourEvent::PorAuth(event) => {
                match event {
                    PorAuthEvent::MutualAuthSuccess {
                        peer_id,
                        connection_id,
                        address,
                        metadata,
                    } => {
                        info!(
                            "✅ Mutual authentication successful with peer {peer_id} at {address}"
                        );
                        // Store authenticated peer
                        self.authenticated_peers.insert(peer_id);

                        // Forward the event to the main application
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::MutualAuthSuccess {
                                    peer_id,
                                    connection_id,
                                    address,
                                    metadata,
                                },
                            })
                            .await;
                    }
                    PorAuthEvent::VerifyPorRequest {
                        peer_id,
                        connection_id,
                        address,
                        por,
                        metadata,
                    } => {
                        info!("🔐 Verification request from peer {peer_id} at {address}");

                        // Forward the verification request to the main application for decision
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::VerifyPorRequest {
                                    peer_id,
                                    connection_id,
                                    address,
                                    por,
                                    metadata,
                                },
                            })
                            .await;

                        // The validation and decision will be made in main.rs
                        // The main application will need to call submit_por_verification_result
                    }
                    PorAuthEvent::OutboundAuthSuccess {
                        peer_id,
                        connection_id,
                        address,
                        metadata,
                    } => {
                        info!("🔹 Outbound authentication successful with {peer_id}");

                        // Forward the event to the main application
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::OutboundAuthSuccess {
                                    peer_id,
                                    connection_id,
                                    address,
                                    metadata,
                                },
                            })
                            .await;
                    }
                    PorAuthEvent::InboundAuthSuccess {
                        peer_id,
                        connection_id,
                        address,
                    } => {
                        info!("🔸 Inbound authentication successful with {peer_id}");

                        // Forward the event to the main application
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::InboundAuthSuccess {
                                    peer_id,
                                    connection_id,
                                    address,
                                },
                            })
                            .await;
                    }
                    PorAuthEvent::OutboundAuthFailure {
                        peer_id,
                        connection_id,
                        address,
                        reason,
                    } => {
                        info!("❌ Outbound authentication failed with {peer_id}: {reason}");

                        // Forward the event to the main application
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::OutboundAuthFailure {
                                    peer_id,
                                    connection_id,
                                    address,
                                    reason,
                                },
                            })
                            .await;
                    }
                    PorAuthEvent::InboundAuthFailure {
                        peer_id,
                        connection_id,
                        address,
                        reason,
                    } => {
                        info!("❌ Inbound authentication failed with {peer_id}: {reason}");

                        // Forward the event to the main application
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::InboundAuthFailure {
                                    peer_id,
                                    connection_id,
                                    address,
                                    reason,
                                },
                            })
                            .await;
                    }
                    PorAuthEvent::AuthTimeout {
                        peer_id,
                        connection_id,
                        address,
                        direction,
                    } => {
                        info!(
                            "⏱️ Authentication timeout with {peer_id}, direction: {:?}",
                            direction
                        );

                        // Forward the event to the main application
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::AuthEvent {
                                event: PorAuthEvent::AuthTimeout {
                                    peer_id,
                                    connection_id,
                                    address,
                                    direction,
                                },
                            })
                            .await;
                    }
                }
            }
            _ => {
             
            }
        }
    }
}

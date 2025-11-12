//! Handler for XRoutesBehaviour
 use crate::xroutes::handler::kad::Addresses;

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use libp2p::{identify, mdns, kad, identity, PeerId, Multiaddr};
use std::collections::HashMap;
use tokio::sync::oneshot;
use tracing::{debug, info};

use super::behaviour::{XRoutesBehaviour, XRoutesBehaviourEvent};
use super::command::XRoutesCommand;
use super::types::{XRoutesConfig, XROUTES_IDENTIFY_PROTOCOL};

/// State for tracking Kademlia operations
#[derive(Default)]
struct KadState {
    /// Pending bootstrap operations
    pending_bootstrap: HashMap<kad::QueryId, oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
    /// Pending find peer operations with target peer_id
    pending_find_peer: HashMap<kad::QueryId, (PeerId, oneshot::Sender<Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>>>)>,
    /// Pending closest peers operations
    pending_closest_peers: HashMap<kad::QueryId, oneshot::Sender<Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>>>>,
}

/// Handler for XRoutesBehaviour
#[derive(Default)]
pub struct XRoutesHandler {
    /// Configuration for the handler
    config: XRoutesConfig,
    /// Local peer ID for enabling behaviours
    local_peer_id: Option<PeerId>,
    /// State for Kademlia operations
    kad_state: KadState,
}

impl XRoutesHandler {
    /// Create a new XRoutesHandler with configuration
    pub fn new(config: XRoutesConfig) -> Self {
        Self { 
            config,
            local_peer_id: None,
            kad_state: KadState::default(),
        }
    }

    /// Create a new XRoutesHandler with local peer ID
    pub fn with_local_peer_id(config: XRoutesConfig, local_peer_id: PeerId) -> Self {
        Self { 
            config,
            local_peer_id: Some(local_peer_id),
            kad_state: KadState::default(),
        }
    }

    /// Set local peer ID
    pub fn set_local_peer_id(&mut self, local_peer_id: PeerId) {
        self.local_peer_id = Some(local_peer_id);
    }

    /// Handle Kademlia events
    async fn handle_kad_event(&mut self, kad_event: kad::Event) {
        match kad_event {
            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                debug!(
                    "🔍 [XRoutesHandler] Kademlia query progressed - ID: {:?}, Result: {:?}",
                    id, result
                );
                
                // Handle query results
                match result {
                    kad::QueryResult::Bootstrap(Ok(_)) => {
                        if let Some(response) = self.kad_state.pending_bootstrap.remove(&id) {
                            let _ = response.send(Ok(()));
                            info!("✅ [XRoutesHandler] Bootstrap completed successfully");
                        }
                    }
                    kad::QueryResult::Bootstrap(Err(e)) => {
                        if let Some(response) = self.kad_state.pending_bootstrap.remove(&id) {
                            let error_msg = format!("{:?}", e);
                            let _ = response.send(Err(e.into()));
                            debug!("❌ [XRoutesHandler] Bootstrap failed: {}", error_msg);
                        }
                    }
                    kad::QueryResult::GetClosestPeers(Ok(peers)) => {
                        // Check if this is a FindPeer or GetClosestPeers query
                        if let Some((target_peer_id, response)) = self.kad_state.pending_find_peer.remove(&id) {
                            // For FindPeer, we need to extract addresses from the peers
                            // and filter only addresses that belong to the target peer
                            let target_peer_id_str = target_peer_id.to_string();
                            let addresses: Vec<Multiaddr> = peers.peers.iter()
                                .flat_map(|peer_info| {
                                    // Extract addresses from PeerInfo
                                    peer_info.addrs.clone()
                                })
                                .filter(|addr| {
                                    // Filter only addresses that contain the target peer ID
                                    addr.to_string().contains(&target_peer_id_str)
                                })
                                .collect();
                            
                            let addresses_len = addresses.len();
                            if addresses.is_empty() {
                                // If no addresses found, return an error - peer not found
                                let error_msg = format!("Peer {} not found in Kademlia DHT", target_peer_id);
                                let _ = response.send(Err(error_msg.into()));
                                info!("❌ [XRoutesHandler] Find peer failed: peer {} not found", target_peer_id);
                            } else {
                                let _ = response.send(Ok(addresses));
                                info!("✅ [XRoutesHandler] Find peer completed with {} addresses for peer: {:?}", addresses_len, target_peer_id);
                            }
                        } else if let Some(response) = self.kad_state.pending_closest_peers.remove(&id) {
                            // For GetClosestPeers, return just the peer IDs
                            let peer_ids: Vec<PeerId> = peers.peers.iter()
                                .map(|peer_info| peer_info.peer_id)
                                .collect();
                            let peer_ids_len = peer_ids.len();
                            let _ = response.send(Ok(peer_ids));
                            info!("✅ [XRoutesHandler] Get closest peers completed with {} peers", peer_ids_len);
                        }
                    }
                    kad::QueryResult::GetClosestPeers(Err(e)) => {
                        if let Some((target_peer_id, response)) = self.kad_state.pending_find_peer.remove(&id) {
                            let error_msg = format!("{:?}", e);
                            let _ = response.send(Err(e.into()));
                            debug!("❌ [XRoutesHandler] Find peer failed for peer {:?}: {}", target_peer_id, error_msg);
                        } else if let Some(response) = self.kad_state.pending_closest_peers.remove(&id) {
                            let error_msg = format!("{:?}", e);
                            let _ = response.send(Err(e.into()));
                            debug!("❌ [XRoutesHandler] Get closest peers failed: {}", error_msg);
                        }
                    }
                    _ => {}
                }
            }
            kad::Event::RoutingUpdated { peer, .. } => {
                debug!(
                    "🔄 [XRoutesHandler] Kademlia routing updated - Peer: {:?}",
                    peer
                );
            }
            kad::Event::UnroutablePeer { peer, .. } => {
                debug!(
                    "❌ [XRoutesHandler] Kademlia unroutable peer - Peer: {:?}",
                    peer
                );
            }
            _ => {}
        }
    }
}

#[async_trait]
impl BehaviourHandler for XRoutesHandler {
    type Behaviour = XRoutesBehaviour;
    type Event = XRoutesBehaviourEvent;
    type Command = XRoutesCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            XRoutesCommand::EnableIdentify { response } => {
                debug!("🔄 [XRoutesHandler] Enabling identify behaviour");
                if let Some(local_peer_id) = self.local_peer_id {
                    // Создаем временный ключ для Identify
                    let keypair = identity::Keypair::generate_ed25519();
                    let public_key = keypair.public();
                    let config = identify::Config::new(XROUTES_IDENTIFY_PROTOCOL.to_string(), public_key);
                    behaviour.enable_identify(config);
                    info!("✅ [XRoutesHandler] Identify behaviour enabled");
                    let _ = response.send(Ok(()));
                } else {
                    debug!("❌ [XRoutesHandler] Cannot enable Identify: local_peer_id not set");
                    let _ = response.send(Err("local_peer_id not set".into()));
                }

            }
            XRoutesCommand::DisableIdentify { response } => {
                debug!("🔄 [XRoutesHandler] Disabling identify behaviour");
                behaviour.disable_identify();
                info!("❌ [XRoutesHandler] Identify behaviour disabled");
                let _ = response.send(Ok(()));
            }
            XRoutesCommand::EnableMdns { response } => {
                debug!("🔄 [XRoutesHandler] Enabling mDNS behaviour");
                if let Some(local_peer_id) = self.local_peer_id {
                    if let Err(e) = behaviour.enable_mdns(local_peer_id) {
                        debug!("❌ [XRoutesHandler] Failed to enable mDNS: {}", e);
                        let _ = response.send(Err(e.into()));
                    } else {
                        info!("✅ [XRoutesHandler] mDNS behaviour enabled");
                        let _ = response.send(Ok(()));
                    }
                } else {
                    debug!("❌ [XRoutesHandler] Cannot enable mDNS: local_peer_id not set");
                    let _ = response.send(Err("local_peer_id not set".into()));
                }
            }
            XRoutesCommand::DisableMdns { response } => {
                debug!("🔄 [XRoutesHandler] Disabling mDNS behaviour");
                behaviour.disable_mdns();
                info!("❌ [XRoutesHandler] mDNS behaviour disabled");
                let _ = response.send(Ok(()));
            }
            XRoutesCommand::EnableKad { response } => {
                debug!("🔄 [XRoutesHandler] Enabling Kademlia behaviour");
                if let Some(local_peer_id) = self.local_peer_id {
                    behaviour.enable_kad(local_peer_id);
                    info!("✅ [XRoutesHandler] Kademlia behaviour enabled");
                    let _ = response.send(Ok(()));
                } else {
                    debug!("❌ [XRoutesHandler] Cannot enable Kademlia: local_peer_id not set");
                    let _ = response.send(Err("local_peer_id not set".into()));
                }
            }
            XRoutesCommand::DisableKad { response } => {
                debug!("🔄 [XRoutesHandler] Disabling Kademlia behaviour");
                behaviour.disable_kad();
                info!("❌ [XRoutesHandler] Kademlia behaviour disabled");
                let _ = response.send(Ok(()));
            }
            XRoutesCommand::GetStatus { response } => {
                debug!("🔄 [XRoutesHandler] Getting XRoutes status");
                let status = behaviour.get_status();
                let _ = response.send(status);
                debug!("📊 [XRoutesHandler] Status sent");
            }
            XRoutesCommand::BootstrapToPeer { peer_id, addresses, response } => {
                println!("🔄 [XRoutesHandler] Bootstrap to peer: {:?}", peer_id);
                if let Some(kad) = behaviour.kad.as_mut() {
                    // Add addresses to Kademlia
                    for addr in &addresses {
                        kad.add_address(&peer_id, addr.clone());
                    }
                    

                     let kbuckets_info = kad.kbuckets();
                    // kbuckets_info — это impl Iterator<Item = KBucketRef<'_, KBucketKey<PeerId>, Addresses>>
                    for (i, bucket) in kbuckets_info.enumerate() {
                        println!("Bucket {}:", i);
                        for entry in bucket.iter() {
                            // entry имеет тип NodeRefView<'_, KBucketKey<PeerId>, Addresses>
                            let peer_id: &PeerId = &entry.node.key.preimage(); // доступ к PeerId
                            let addresses: &Addresses = &entry.node.value;   // доступ к адресам
                            println!("  PeerId: {:?}", peer_id);
                            println!("  Addresses: {:?}", addresses);
                        }
                    }
                    
                    
                    // Start bootstrap
                    match kad.bootstrap() {
                        Ok(query_id) => {
                            self.kad_state.pending_bootstrap.insert(query_id, response);
                            println!("✅ [XRoutesHandler] Bootstrap started for peer: {:?}", peer_id);
                        }
                        Err(e) => {
                            let error_msg = format!("{:?}", e);
                            let _ = response.send(Err(e.into()));
                            println!("❌ [XRoutesHandler] Bootstrap failed to start: {}", error_msg);
                        }
                    }
                } else {
                    let _ = response.send(Err("Kademlia behaviour not enabled".into()));
                    println!("❌ [XRoutesHandler] Cannot bootstrap: Kademlia not enabled");
                }
            }
            XRoutesCommand::FindPeer { peer_id, response } => {
                debug!("🔄 [XRoutesHandler] Find peer: {:?}", peer_id);
                if let Some(kad) = behaviour.kad.as_mut() {
                    let query_id = kad.get_closest_peers(peer_id);
                    self.kad_state.pending_find_peer.insert(query_id, (peer_id, response));
                    info!("✅ [XRoutesHandler] Find peer started for: {:?}", peer_id);
                } else {
                    let _ = response.send(Err("Kademlia behaviour not enabled".into()));
                    debug!("❌ [XRoutesHandler] Cannot find peer: Kademlia not enabled");
                }
            }
            XRoutesCommand::GetClosestPeers { peer_id, response } => {
                debug!("🔄 [XRoutesHandler] Get closest peers for peer: {:?}", peer_id);
                if let Some(kad) = behaviour.kad.as_mut() {
                    let query_id = kad.get_closest_peers(peer_id);
                    self.kad_state.pending_closest_peers.insert(query_id, response);
                    info!("✅ [XRoutesHandler] Get closest peers started for: {:?}", peer_id);
                } else {
                    let _ = response.send(Err("Kademlia behaviour not enabled".into()));
                    debug!("❌ [XRoutesHandler] Cannot get closest peers: Kademlia not enabled");
                }
            }
            XRoutesCommand::FindPeerAddresses { peer_id, timeout, response } => {
                debug!("🔄 [XRoutesHandler] Find peer addresses with timeout: {:?} for peer: {:?}", timeout, peer_id);
                if let Some(kad) = behaviour.kad.as_mut() {
                    // For now, we'll always initiate search since we don't have direct access to local addresses
                    // In a real implementation, we might check the routing table or use a different approach
                    let query_id = kad.get_closest_peers(peer_id);
                    self.kad_state.pending_find_peer.insert(query_id, (peer_id, response));
                    info!("✅ [XRoutesHandler] Find peer addresses started for: {:?} with timeout: {:?}", peer_id, timeout);
                } else {
                    let _ = response.send(Err("Kademlia behaviour not enabled".into()));
                    debug!("❌ [XRoutesHandler] Cannot find peer addresses: Kademlia not enabled");
                }
            }
        }
    }

    async fn handle_event(&mut self, behaviour: &mut Self::Behaviour, event: &Self::Event) {
        match event {
            XRoutesBehaviourEvent::Identify(identify_event) => {
                match identify_event {
                    identify::Event::Received { peer_id, info, .. } => {
                        debug!(
                            "📨 [XRoutesHandler] Identify received - Peer: {:?}, Protocols: {:?}",
                            peer_id, info.protocols
                        );
                        // Печатаем событие IdentifyReceived
                        println!("🆔 [XRoutesHandler] IdentifyReceived - Peer: {:?}, Addresses: {:?} {:?}", 
                                 peer_id, info.listen_addrs, identify_event);

                        // Добавляем адреса в Kademlia DHT
                        if let Some(kad) = behaviour.kad.as_mut() {
                            for addr in &info.listen_addrs {
                                kad.add_address(&peer_id, addr.clone());
                                println!("📍 [XRoutesHandler] Added address to Kademlia: {} -> {}", peer_id, addr);
                            }
                            println!("✅ [XRoutesHandler] Added {} addresses to Kademlia for peer: {}", 
                                     info.listen_addrs.len(), peer_id);
                        } else {
                            debug!("⚠️ [XRoutesHandler] Kademlia not enabled, cannot add addresses for peer: {}", peer_id);
                        }

                        if let Some(kad) = behaviour.kad.as_mut() {
                    
                                        let kbuckets_info = kad.kbuckets();
                    // kbuckets_info — это impl Iterator<Item = KBucketRef<'_, KBucketKey<PeerId>, Addresses>>
                    for (i, bucket) in kbuckets_info.enumerate() {
                        println!("Bucket {}:", i);
                        for entry in bucket.iter() {
                            // entry имеет тип NodeRefView<'_, KBucketKey<PeerId>, Addresses>
                            let peer_id: &PeerId = &entry.node.key.preimage(); // доступ к PeerId
                            let addresses: &Addresses = &entry.node.value;   // доступ к адресам
                            println!("  PeerId: {:?}", peer_id);
                            println!("  Addresses: {:?}", addresses);
                        }
                    }}
                    




                    }
                    identify::Event::Sent { peer_id, .. } => {
                        debug!(
                            "📤 [XRoutesHandler] Identify sent to peer: {:?}",
                            peer_id
                        );
                        // Печатаем событие IdentifySent
                        println!("🆔 [XRoutesHandler] IdentifySent - Peer: {:?}", peer_id);
                    }
                    identify::Event::Error { peer_id, error, .. } => {
                        debug!(
                            "❌ [XRoutesHandler] Identify error with peer {:?}: {}",
                            peer_id, error
                        );
                        // Печатаем событие IdentifyError
                        println!("❌ [XRoutesHandler] IdentifyError - Peer: {:?}, Error: {}", peer_id, error);
                    }
                    _ => {}
                }
            }
            XRoutesBehaviourEvent::Mdns(mdns_event) => {
                match mdns_event {
                    mdns::Event::Discovered(list) => {
                        debug!(
                            "🔍 [XRoutesHandler] mDNS discovered {} peers",
                            list.len()
                        );
                        for (peer_id, addr) in list {
                            debug!(
                                "   - Peer: {:?}, Address: {:?}",
                                peer_id, addr
                            );
                        }
                    }
                    mdns::Event::Expired(list) => {
                        debug!(
                            "🗑️ [XRoutesHandler] mDNS expired {} peers",
                            list.len()
                        );
                        for (peer_id, addr) in list {
                            debug!(
                                "   - Peer: {:?}, Address: {:?}",
                                peer_id, addr
                            );
                        }
                    }
                }
            }
            XRoutesBehaviourEvent::Kad(kad_event) => {
                self.handle_kad_event(kad_event.clone()).await;
            }
        }
    }
}

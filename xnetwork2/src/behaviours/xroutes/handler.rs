//! Handler for XRoutesBehaviour
 use crate::xroutes::handler::kad::Addresses;

use async_trait::async_trait;
use command_swarm::BehaviourHandler;
use libp2p::identity::PublicKey;
use libp2p::{identify, mdns, kad, identity, PeerId, Multiaddr};
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use tokio::sync::oneshot;
use tracing::{debug, info};

use super::behaviour::{XRoutesBehaviour, XRoutesBehaviourEvent};
use super::command::{XRoutesCommand, MdnsCacheStatus};
use super::pending_task_manager::PendingTaskManager;
use super::types::{XRoutesConfig, XROUTES_IDENTIFY_PROTOCOL};

/// Record for mDNS peer with TTL
#[derive(Debug, Clone)]
struct MdnsPeerRecord {
    /// Peer addresses
    addresses: Vec<Multiaddr>,
    /// When this record was last updated
    last_updated: SystemTime,
    /// TTL in seconds
    ttl_seconds: u64,
}

impl MdnsPeerRecord {
    /// Create a new mDNS peer record
    fn new(addresses: Vec<Multiaddr>, ttl_seconds: u64) -> Self {
        Self {
            addresses,
            last_updated: SystemTime::now(),
            ttl_seconds,
        }
    }

    /// Check if the record has expired
    fn is_expired(&self) -> bool {
        match SystemTime::now().duration_since(self.last_updated) {
            Ok(elapsed) => elapsed.as_secs() >= self.ttl_seconds,
            Err(_) => true, // If time went backwards, consider expired
        }
    }

    /// Update the record with new addresses
    fn update(&mut self, addresses: Vec<Multiaddr>) {
        self.addresses = addresses;
        self.last_updated = SystemTime::now();
    }
}

/// State for tracking mDNS operations
struct MdnsState {
    /// Cache of mDNS discovered peers
    peer_cache: HashMap<PeerId, MdnsPeerRecord>,
    /// Default TTL for mDNS records in seconds
    default_ttl_seconds: u64,
}

impl Default for MdnsState {
    fn default() -> Self {
        Self {
            peer_cache: HashMap::new(),
            default_ttl_seconds: 60, // 1 minute default TTL
        }
    }
}

/// State for tracking Kademlia operations
struct KadState {
    /// Pending bootstrap operations
    pending_bootstrap: HashMap<kad::QueryId, oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
    /// Pending find peer operations with target peer_id
    pending_find_peer: HashMap<kad::QueryId, (PeerId, oneshot::Sender<Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>>>)>,
    /// Pending closest peers operations
    pending_closest_peers: HashMap<kad::QueryId, oneshot::Sender<Result<Vec<PeerId>, Box<dyn std::error::Error + Send + Sync>>>>,
    /// Pending tasks for find peer addresses operations with timeout
    find_addresses_tasks: PendingTaskManager<
        kad::QueryId, 
        Vec<Multiaddr>, 
        Box<dyn std::error::Error + Send + Sync>,
        PeerId  // Extra тип - целевой peer_id
    >,
}

impl Default for KadState {
    fn default() -> Self {
        Self {
            pending_bootstrap: HashMap::new(),
            pending_find_peer: HashMap::new(),
            pending_closest_peers: HashMap::new(),
            find_addresses_tasks: PendingTaskManager::new(),
        }
    }
}

/// Handler for XRoutesBehaviour
pub struct XRoutesHandler {
    /// Configuration for the handler
    config: XRoutesConfig,
    /// Local peer ID for enabling behaviours
    local_peer_id: PeerId,
    local_public_key: PublicKey,
    /// State for mDNS operations
    mdns_state: MdnsState,
    /// State for Kademlia operations
    kad_state: KadState,
}

impl XRoutesHandler {
    /// Create a new XRoutesHandler with configuration
    pub fn new(local_public_key: PublicKey, config: XRoutesConfig) -> Self {
        let local_peer_id = local_public_key.to_peer_id();
        Self {
            local_public_key, 
            config,
            local_peer_id: local_peer_id,
            mdns_state: MdnsState::default(),
            kad_state: KadState::default(),
        }
    }


    /// Handle mDNS discovered event
    fn handle_mdns_discovered(&mut self, list: &Vec<(PeerId, Multiaddr)>, behaviour: &mut XRoutesBehaviour) {
        debug!("🔍 [XRoutesHandler] Processing mDNS discovered {} peers", list.len());
        
        // Group addresses by peer_id
        let mut peers_by_id: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
        for (peer_id, addr) in list {
            peers_by_id.entry(*peer_id).or_default().push(addr.clone());
        }

        // Update cache and add to Kademlia if enabled
        for (peer_id, addresses) in peers_by_id {
            // Update cache
            let record = MdnsPeerRecord::new(addresses.clone(), self.mdns_state.default_ttl_seconds);
            self.mdns_state.peer_cache.insert(peer_id, record);

            // Add to Kademlia if enabled
            if let Some(kad) = behaviour.kad.as_mut() {
                for addr in &addresses {
                    kad.add_address(&peer_id, addr.clone());
                    debug!("📍 [XRoutesHandler] Added mDNS peer to Kademlia: {} -> {}", peer_id, addr);
                }
                info!("✅ [XRoutesHandler] Added mDNS peer {} with {} addresses to Kademlia", peer_id, addresses.len());
            }

            info!("✅ [XRoutesHandler] mDNS peer discovered: {} with {} addresses", peer_id, addresses.len());
        }
    }

    /// Handle mDNS expired event
    fn handle_mdns_expired(&mut self, list: &Vec<(PeerId, Multiaddr)>) {
        debug!("🗑️ [XRoutesHandler] Processing mDNS expired {} peers", list.len());
        
        // Group addresses by peer_id
        let mut peers_by_id: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
        for (peer_id, addr) in list {
            peers_by_id.entry(*peer_id).or_default().push(addr.clone());
        }

        // Remove expired peers from cache
        for (peer_id, _) in peers_by_id {
            if self.mdns_state.peer_cache.remove(&peer_id).is_some() {
                info!("🗑️ [XRoutesHandler] Removed expired mDNS peer: {}", peer_id);
            }
        }
    }

    /// Clean expired mDNS records
    fn clean_expired_mdns_records(&mut self) -> usize {
        let before_count = self.mdns_state.peer_cache.len();
        self.mdns_state.peer_cache.retain(|_, record| !record.is_expired());
        let after_count = self.mdns_state.peer_cache.len();
        let removed_count = before_count - after_count;
        
        if removed_count > 0 {
            debug!("🧹 [XRoutesHandler] Cleaned {} expired mDNS records", removed_count);
        }
        
        removed_count
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
                        // Сначала проверяем задачи FindPeerAddresses с таймаутом
                        if let Some(target_peer_id) = self.kad_state.find_addresses_tasks.get_task_extra(&id) {
                            // Для FindPeerAddresses, извлекаем адреса из peers и фильтруем только для целевого peer_id
                            let target_peer_id_str = target_peer_id.to_string();
                            let addresses: Vec<Multiaddr> = peers.peers.iter()
                                .flat_map(|peer_info| {
                                    // Извлекаем адреса из PeerInfo
                                    peer_info.addrs.clone()
                                })
                                .filter(|addr| {
                                    // Фильтруем только адреса, которые содержат целевой peer ID
                                    addr.to_string().contains(&target_peer_id_str)
                                })
                                .collect();
                            
                            let addresses_len = addresses.len();
                            if addresses.is_empty() {
                                // Если адреса не найдены, устанавливаем ошибку - peer не найден
                                let error_msg = format!("Peer {} not found in Kademlia DHT", target_peer_id);
                                let _ = self.kad_state.find_addresses_tasks.set_task_error(&id, error_msg.into());
                                info!("❌ [XRoutesHandler] Find peer addresses failed: peer {} not found", target_peer_id);
                            } else {
                                let _ = self.kad_state.find_addresses_tasks.set_task_result(&id, addresses);
                                info!("✅ [XRoutesHandler] Find peer addresses completed with {} addresses for peer: {:?}", addresses_len, target_peer_id);
                            }
                        }
                        // Затем проверяем обычные операции FindPeer
                        else if let Some((target_peer_id, response)) = self.kad_state.pending_find_peer.remove(&id) {
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
                        // Сначала проверяем задачи FindPeerAddresses с таймаутом
                        if let Some(target_peer_id) = self.kad_state.find_addresses_tasks.get_task_extra(&id) {
                            let error_msg = format!("{:?}", e);
                            let _ = self.kad_state.find_addresses_tasks.set_task_error(&id, e.into());
                            debug!("❌ [XRoutesHandler] Find peer addresses failed for peer {:?}: {}", target_peer_id, error_msg);
                        }
                        // Затем проверяем обычные операции FindPeer
                        else if let Some((target_peer_id, response)) = self.kad_state.pending_find_peer.remove(&id) {
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
                behaviour.enable_identify(self.local_public_key.clone());
                info!("✅ [XRoutesHandler] Identify behaviour enabled");
                let _ = response.send(Ok(()));

            }
            XRoutesCommand::DisableIdentify { response } => {
                debug!("🔄 [XRoutesHandler] Disabling identify behaviour");
                behaviour.disable_identify();
                info!("❌ [XRoutesHandler] Identify behaviour disabled");
                let _ = response.send(Ok(()));
            }
            XRoutesCommand::EnableMdns { response } => {
                debug!("🔄 [XRoutesHandler] Enabling mDNS behaviour");
            
                if let Err(e) = behaviour.enable_mdns(self.local_peer_id) {
                    debug!("❌ [XRoutesHandler] Failed to enable mDNS: {}", e);
                    let _ = response.send(Err(e.into()));
                } else {
                    info!("✅ [XRoutesHandler] mDNS behaviour enabled");
                    let _ = response.send(Ok(()));
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
                behaviour.enable_kad(self.local_peer_id);
                info!("✅ [XRoutesHandler] Kademlia behaviour enabled");
                let _ = response.send(Ok(()));
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
                    // Инициируем поиск в Kademlia
                    let query_id = kad.get_closest_peers(peer_id);
                    
                    // Добавляем задачу в PendingTaskManager с таймаутом и целевым peer_id как extra данными
                    self.kad_state.find_addresses_tasks.add_pending_task_with_extra(
                        query_id,
                        timeout,
                        response,
                        peer_id,
                    );
                    
                    info!("✅ [XRoutesHandler] Find peer addresses started for: {:?} with timeout: {:?} (query_id: {:?})", peer_id, timeout, query_id);
                } else {
                    let _ = response.send(Err("Kademlia behaviour not enabled".into()));
                    debug!("❌ [XRoutesHandler] Cannot find peer addresses: Kademlia not enabled");
                }
            }
            XRoutesCommand::GetMdnsPeers { response } => {
                debug!("🔄 [XRoutesHandler] Getting all mDNS peers from cache");
                
                // Clean expired records first
                self.clean_expired_mdns_records();
                
                // Collect all peers and their addresses
                let peers: Vec<(PeerId, Vec<Multiaddr>)> = self.mdns_state.peer_cache
                    .iter()
                    .map(|(peer_id, record)| (peer_id.clone(), record.addresses.clone()))
                    .collect();
                
                info!("✅ [XRoutesHandler] Returning {} mDNS peers from cache", peers.len());
                let _ = response.send(Ok(peers));
            }
            XRoutesCommand::FindMdnsPeer { peer_id, response } => {
                debug!("🔄 [XRoutesHandler] Finding specific peer in mDNS cache: {:?}", peer_id);
                
                // Clean expired records first
                self.clean_expired_mdns_records();
                
                // Find the peer in cache
                let addresses = self.mdns_state.peer_cache
                    .get(&peer_id)
                    .map(|record| record.addresses.clone());
                
                if addresses.is_some() {
                    info!("✅ [XRoutesHandler] Found peer {:?} in mDNS cache", peer_id);
                } else {
                    debug!("❌ [XRoutesHandler] Peer {:?} not found in mDNS cache", peer_id);
                }
                
                let _ = response.send(Ok(addresses));
            }
            XRoutesCommand::GetMdnsCacheStatus { response } => {
                debug!("🔄 [XRoutesHandler] Getting mDNS cache status");
                
                // Clean expired records first
                let cleaned_count = self.clean_expired_mdns_records();
                
                let status = MdnsCacheStatus {
                    total_peers: self.mdns_state.peer_cache.len(),
                    cache_size: self.mdns_state.peer_cache.len(),
                    last_update: SystemTime::now(),
                    ttl_seconds: self.mdns_state.default_ttl_seconds,
                };
                
                info!("✅ [XRoutesHandler] mDNS cache status: {} peers, cleaned {} expired", 
                      status.total_peers, cleaned_count);
                let _ = response.send(Ok(status));
            }
            XRoutesCommand::ClearMdnsCache { response } => {
                debug!("🔄 [XRoutesHandler] Clearing mDNS cache");
                
                let cleared_count = self.mdns_state.peer_cache.len();
                self.mdns_state.peer_cache.clear();
                
                info!("✅ [XRoutesHandler] Cleared {} entries from mDNS cache", cleared_count);
                let _ = response.send(Ok(cleared_count));
            }
            XRoutesCommand::EnableMdnsWithTtl { ttl_seconds, response } => {
                debug!("🔄 [XRoutesHandler] Enabling mDNS with custom TTL: {} seconds", ttl_seconds);
                
            
                // Set custom TTL
                self.mdns_state.default_ttl_seconds = ttl_seconds;
                
                if let Err(e) = behaviour.enable_mdns(self.local_peer_id) {
                    debug!("❌ [XRoutesHandler] Failed to enable mDNS with TTL: {}", e);
                    let _ = response.send(Err(e.into()));
                } else {
                    info!("✅ [XRoutesHandler] mDNS behaviour enabled with TTL: {} seconds", ttl_seconds);
                    let _ = response.send(Ok(()));
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
                        self.handle_mdns_discovered(list, behaviour);
                    }
                    mdns::Event::Expired(list) => {
                        self.handle_mdns_expired(list);
                    }
                }
            }
            XRoutesBehaviourEvent::Kad(kad_event) => {
                self.handle_kad_event(kad_event.clone()).await;
            }
            XRoutesBehaviourEvent::RelayServer(relay_event) => {
                // TODO: Add relay server event handling

            }
            XRoutesBehaviourEvent::RelayClient(relay_client_event) => {
                // TODO: Add relay client event handling
            }
        }
    }
}

// src/xroutes/handler.rs

use libp2p::{identity, kad, mdns, Multiaddr, PeerId, Swarm};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{info, warn, debug, error};

use crate::behaviour::NodeBehaviour;

use super::{
    commands::XRoutesCommand,
    events::XRoutesEvent,
    types::{BootstrapNodeInfo, BootstrapError},
    xroute::XRouteRole,
    behaviour::XRoutesDiscoveryBehaviourEvent,
    XRoutesConfig,
};

// Structure to track individual search waiters with their own timeouts
#[derive(Debug)]
struct SearchWaiter {
    response_channel: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    timeout_handle: Option<tokio::task::AbortHandle>,
    start_time: Instant,
    timeout_secs: i32,
    waiter_id: u64,
}

// Structure to track search state for a specific peer
#[derive(Debug)]
struct SearchState {
    query_id: Option<kad::QueryId>,
    waiters: Vec<SearchWaiter>,
    peer_id: PeerId,
    created_at: Instant,
}

pub struct XRoutesHandler {
    current_role: XRouteRole,
    mdns_enabled: bool,
    kad_enabled: bool,
    bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
    discovered_peers: HashMap<PeerId, Vec<Multiaddr>>,
    config: XRoutesConfig,
    
    // New fields for advanced peer search
    active_searches: HashMap<PeerId, SearchState>,
    next_waiter_id: u64,
    kad_query_to_peer: HashMap<kad::QueryId, PeerId>, // Map query IDs to peer IDs
}

impl XRoutesHandler {
    pub fn new(config: XRoutesConfig) -> Self {
        Self {
            current_role: config.initial_role.clone(),
            mdns_enabled: config.enable_mdns,
            kad_enabled: config.enable_kad,
            bootstrap_nodes: Vec::new(),
            discovered_peers: HashMap::new(),
            config,
            active_searches: HashMap::new(),
            next_waiter_id: 1,
            kad_query_to_peer: HashMap::new(),
        }
    }

    /// Handle XRoutes commands
    pub async fn handle_command(
        &mut self,
        cmd: XRoutesCommand,
        swarm: &mut Swarm<NodeBehaviour>,
        key: &identity::Keypair,
    ) {
        match cmd {

            XRoutesCommand::DiscoveryCommand(discover_cmd) => {
                //noop
            }
            XRoutesCommand::EnableMdns => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    match xroutes.enable_mdns(key) {
                        Ok(_) => {
                            self.mdns_enabled = true;
                            info!("mDNS discovery enabled");
                        }
                        Err(e) => {
                            warn!("Failed to enable mDNS: {}", e);
                        }
                    }
                }
            }

            XRoutesCommand::DisableMdns => {
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.disable_mdns();
                    self.mdns_enabled = false;
                    info!("mDNS discovery disabled");
                }
            }

            XRoutesCommand::EnableKad => {
                self.kad_enabled = true;
                info!("Kademlia discovery enabled");
                if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    if let Err(e) = xroutes.bootstrap() {
                        warn!("Failed to bootstrap Kademlia: {}", e);
                    } else {
                        info!("Bootstrap query initiated");
                    }
                }
            }

            XRoutesCommand::DisableKad => {
                self.kad_enabled = false;
                info!("Kademlia discovery disabled");
                // Cancel all active searches
                self.cancel_all_searches("Kademlia disabled").await;
            }

            XRoutesCommand::BootstrapKad { response } => {
                info!("Bootstrapping Kademlia DHT");
                let result = if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    match xroutes.bootstrap() {
                        Ok(query_id) => {
                            info!("Kademlia bootstrap started successfully with query_id: {:?}", query_id);
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Failed to bootstrap Kademlia: {}", e);
                            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        }
                    }
                } else {
                    Err("XRoutes discovery not enabled".into())
                };
                let _ = response.send(result);
            }

            XRoutesCommand::GetKadKnownPeers { response } => {
                let peers = if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                    xroutes.get_known_peers()
                } else {
                    Vec::new()
                };
                let _ = response.send(peers);
            }

            XRoutesCommand::GetPeerAddresses { peer_id, response } => {
                let addresses = self.get_local_peer_addresses(&peer_id, swarm);
                let _ = response.send(addresses);
            }

            XRoutesCommand::FindPeerAddresses { peer_id, response } => {
                // Legacy method - use 30 second timeout
                let (tx, rx) = oneshot::channel();
                self.handle_find_peer_addresses_advanced(peer_id, 30, tx, swarm).await;
                
                let result = match rx.await {
                    Ok(Ok(_addresses)) => Ok(()),
                    Ok(Err(e)) => Err(e.into()),
                    Err(e) => Err(format!("Internal error: {}", e).into()),
                };
                let _ = response.send(result);
            }

            // NEW: Advanced peer address finding
            XRoutesCommand::FindPeerAddressesAdvanced { peer_id, timeout_secs, response } => {
                self.handle_find_peer_addresses_advanced(peer_id, timeout_secs, response, swarm).await;
            }

            // NEW: Cancel peer search
            XRoutesCommand::CancelPeerSearch { peer_id, response } => {
                let result = self.cancel_peer_search(&peer_id).await;
                let _ = response.send(result);
            }

            // NEW: Get active searches
            XRoutesCommand::GetActiveSearches { response } => {
                let searches = self.get_active_searches_info();
                let _ = response.send(searches);
            }

            XRoutesCommand::SetRole { role, response } => {
                let result = self.set_role(role, swarm).await;
                let _ = response.send(result);
            }

            XRoutesCommand::GetRole { response } => {
                let _ = response.send(self.current_role.clone());
            }
            
            XRoutesCommand::ConnectToBootstrap { addr, timeout_secs, response } => {
                let result = self.handle_bootstrap_connection(addr, timeout_secs, swarm).await;
                let _ = response.send(result);
            }
        }
    }

    /// Handle advanced peer address finding with individual timeouts
    async fn handle_find_peer_addresses_advanced(
        &mut self,
        peer_id: PeerId,
        timeout_secs: i32,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
        swarm: &mut Swarm<NodeBehaviour>,
    ) {
        debug!("Finding addresses for peer {} with timeout {}", peer_id, timeout_secs);

        // Step 1: Check local tables first
        let local_addresses = self.get_local_peer_addresses(&peer_id, swarm);
        
        // Step 2: If timeout is 0 or we found addresses locally, return immediately
        if timeout_secs == 0 || !local_addresses.is_empty() {
            debug!("Returning {} local addresses for peer {}", local_addresses.len(), peer_id);
            let _ = response.send(Ok(local_addresses));
            return;
        }

        // Step 3: Check if Kademlia is enabled
        if !self.kad_enabled {
            let _ = response.send(Err("Kademlia not enabled, cannot perform DHT search".to_string()));
            return;
        }

        // Step 4: Create search waiter
        let waiter = self.create_search_waiter(response, timeout_secs, peer_id).await;

        // Step 5: Add to existing search or create new one
        if let Some(search_state) = self.active_searches.get_mut(&peer_id) {
            debug!("Adding waiter to existing search for peer {}", peer_id);
            search_state.waiters.push(waiter);
        } else {
            debug!("Creating new search for peer {}", peer_id);
            let query_id = self.initiate_kad_search(&peer_id, swarm);
            
            let search_state = SearchState {
                query_id,
                waiters: vec![waiter],
                peer_id,
                created_at: Instant::now(),
            };
            
            // Map query ID to peer ID for later lookup
            if let Some(qid) = query_id {
                self.kad_query_to_peer.insert(qid, peer_id);
            }
            
            self.active_searches.insert(peer_id, search_state);
        }
    }

    /// Create a search waiter with timeout handling
    async fn create_search_waiter(
        &mut self,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
        timeout_secs: i32,
        peer_id: PeerId,
    ) -> SearchWaiter {
        let waiter_id = self.next_waiter_id;
        self.next_waiter_id += 1;

        let timeout_handle = if timeout_secs > 0 {
            // Create a separate channel for timeout communication
            let (timeout_tx, timeout_rx) = oneshot::channel::<()>();
            
            // Create timeout task
            let timeout_duration = Duration::from_secs(timeout_secs as u64);
            let handle = tokio::spawn(async move {
                tokio::select! {
                    _ = tokio::time::sleep(timeout_duration) => {
                        debug!("Search timeout for peer {} (waiter {})", peer_id, waiter_id);
                        // Don't send here - we'll handle timeout in the main logic
                    }
                    _ = timeout_rx => {
                        debug!("Search timeout cancelled for peer {} (waiter {})", peer_id, waiter_id);
                    }
                }
            });
            
            Some(handle.abort_handle())
        } else {
            None // timeout_secs == -1, infinite search
        };

        SearchWaiter {
            response_channel: response,
            timeout_handle,
            start_time: Instant::now(),
            timeout_secs,
            waiter_id,
        }
    }

    /// Get addresses from local tables (Kademlia + mDNS cache)
    fn get_local_peer_addresses(
        &self,
        peer_id: &PeerId,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<Multiaddr> {
        let mut addresses = Vec::new();

        // Get from Kademlia routing table
        if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
            let kad_addresses = xroutes.get_peer_addresses(peer_id);
            addresses.extend(kad_addresses);
        }

        // Get from mDNS discovered peers cache
        if let Some(mdns_addresses) = self.discovered_peers.get(peer_id) {
            addresses.extend(mdns_addresses.clone());
        }

        // Deduplicate addresses
        addresses.sort();
        addresses.dedup();

        debug!("Found {} local addresses for peer {}", addresses.len(), peer_id);
        addresses
    }

    /// Initiate Kademlia search for peer
    fn initiate_kad_search(
        &self,
        peer_id: &PeerId,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Option<kad::QueryId> {
        if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
            debug!("Initiating Kademlia search for peer {}", peer_id);
            Some(xroutes.kad.get_closest_peers(*peer_id))
        } else {
            warn!("Cannot initiate Kademlia search - XRoutes not available");
            None
        }
    }

    /// Cancel search for specific peer
    async fn cancel_peer_search(&mut self, peer_id: &PeerId) -> Result<(), String> {
        if let Some(search_state) = self.active_searches.remove(peer_id) {
            info!("Cancelling search for peer {} with {} waiters", peer_id, search_state.waiters.len());
            
            // Cancel all waiters
            for waiter in search_state.waiters {
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                let _ = waiter.response_channel.send(Err("Search cancelled".to_string()));
            }
            
            // Remove query ID mapping
            if let Some(query_id) = search_state.query_id {
                self.kad_query_to_peer.remove(&query_id);
            }
            
            Ok(())
        } else {
            Err(format!("No active search found for peer {}", peer_id))
        }
    }

    /// Cancel all active searches
    async fn cancel_all_searches(&mut self, reason: &str) {
        let peer_ids: Vec<PeerId> = self.active_searches.keys().cloned().collect();
        
        for peer_id in peer_ids {
            if let Some(search_state) = self.active_searches.remove(&peer_id) {
                for waiter in search_state.waiters {
                    if let Some(handle) = waiter.timeout_handle {
                        handle.abort();
                    }
                    let _ = waiter.response_channel.send(Err(reason.to_string()));
                }
            }
        }
        
        self.kad_query_to_peer.clear();
        info!("Cancelled all active searches: {}", reason);
    }

    /// Clean up timed out waiters for all active searches
    pub fn cleanup_timed_out_waiters(&mut self) {
        let mut peers_to_remove = Vec::new();
        let mut total_cleaned = 0;
        
        for (peer_id, search_state) in self.active_searches.iter_mut() {
            let mut active_waiters = Vec::new();
            let mut timed_out_waiters = Vec::new();
            
            // Separate active and timed out waiters
            for waiter in search_state.waiters.drain(..) {
                if waiter.timeout_secs > 0 && waiter.start_time.elapsed().as_secs() >= waiter.timeout_secs as u64 {
                    timed_out_waiters.push(waiter);
                } else {
                    active_waiters.push(waiter);
                }
            }
            
            // Send timeout errors to timed out waiters
            for waiter in timed_out_waiters {
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                let _ = waiter.response_channel.send(Err("Search timeout".to_string()));
                total_cleaned += 1;
            }
            
            // Update waiters list
            search_state.waiters = active_waiters;
            
            // Mark for removal if no active waiters left
            if search_state.waiters.is_empty() {
                peers_to_remove.push(*peer_id);
            }
        }
        
        // Remove searches with no active waiters
        for peer_id in peers_to_remove {
            if let Some(search_state) = self.active_searches.remove(&peer_id) {
                if let Some(query_id) = search_state.query_id {
                    self.kad_query_to_peer.remove(&query_id);
                }
                debug!("Removed search for peer {} - all waiters timed out", peer_id);
            }
        }

        if total_cleaned > 0 {
            info!("Cleaned up {} timed out search waiters", total_cleaned);
        }
    }
    /// Get information about active searches
    pub fn get_active_searches_info(&self) -> Vec<(PeerId, usize, Duration)> {
        self.active_searches
            .iter()
            .map(|(peer_id, search_state)| {
                let duration = search_state.created_at.elapsed();
                let waiters_count = search_state.waiters.len();
                (*peer_id, waiters_count, duration)
            })
            .collect()
    }

    /// Handle XRoutes behaviour events and return network events
    pub async fn handle_behaviour_event(
        &mut self,
        event: XRoutesDiscoveryBehaviourEvent,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        match event {
            XRoutesDiscoveryBehaviourEvent::Mdns(mdns_event) => {
                self.handle_mdns_event(mdns_event, swarm).await
            }
            XRoutesDiscoveryBehaviourEvent::Kad(kad_event) => {
                self.handle_kad_event(kad_event, swarm).await
            }
        }
    }

    /// Handle mDNS events
    async fn handle_mdns_event(
        &mut self,
        event: mdns::Event,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        let mut events = Vec::new();

        match event {
            mdns::Event::Discovered(peers) => {
                if self.mdns_enabled {
                    let mut discovered_addresses = Vec::new();
                    
                    for (peer_id, addr) in peers {
                        info!("mDNS discovered peer: {peer_id} at {addr}");
                        
                        // Add to our discovered peers map
                        self.discovered_peers
                            .entry(peer_id)
                            .or_insert_with(Vec::new)
                            .push(addr.clone());
                        
                        discovered_addresses.push((peer_id, addr.clone()));

                        // Check if we have active searches for this peer
                        if let Some(search_state) = self.active_searches.get(&peer_id) {
                            let addresses = vec![addr.clone()];
                            self.complete_search_for_peer(peer_id, addresses).await;
                        }

                        // Add to Kademlia if enabled
                        if self.kad_enabled {
                            if let Some(ref mut xroutes) = swarm.behaviour_mut().xroutes.as_mut() {
                                xroutes.add_address(&peer_id, addr.clone());
                                info!("📚 Added address to Kademlia: {peer_id} at {addr}");
                                
                                events.push(XRoutesEvent::KadAddressAdded {
                                    peer_id,
                                    addr: addr.clone(),
                                });
                            }
                        }
                    }

                    // Group discovered peers by peer_id
                    let mut peer_groups: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
                    for (peer_id, addr) in discovered_addresses {
                        peer_groups.entry(peer_id).or_insert_with(Vec::new).push(addr);
                    }

                    // Generate discovery events
                    for (peer_id, addresses) in peer_groups {
                        events.push(XRoutesEvent::MdnsPeerDiscovered {
                            peer_id,
                            addresses,
                        });
                    }
                }
            }
            mdns::Event::Expired(peers) => {
                for (peer_id, _addr) in peers {
                    info!("mDNS peer expired: {peer_id}");
                    self.discovered_peers.remove(&peer_id);
                    events.push(XRoutesEvent::MdnsPeerExpired { peer_id });
                }
            }
        }

        events
    }

    /// Handle Kademlia events
    async fn handle_kad_event(
        &mut self,
        event: kad::Event,
        _swarm: &mut Swarm<NodeBehaviour>,
    ) -> Vec<XRoutesEvent> {
        let mut events = Vec::new();

        match event {
            kad::Event::RoutingUpdated { peer, addresses, .. } => {
                info!("📔 Kademlia routing updated for peer: {peer}");
                for addr in addresses.iter() {
                    info!("📕 Known address: {addr}");
                }

                // Check if we have active searches for this peer
                if self.active_searches.contains_key(&peer) {
                    let addresses_vec: Vec<Multiaddr> = addresses.iter().cloned().collect();
                    if !addresses_vec.is_empty() {
                        self.complete_search_for_peer(peer, addresses_vec.clone()).await;
                    }
                }

                events.push(XRoutesEvent::KadRoutingUpdated {
                    peer_id: peer,
                    addresses: addresses.iter().cloned().collect(),
                });
            }

            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                match result {
                    kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { peers, .. })) => {
                        // Find which peer this query was for
                        if let Some(target_peer_id) = self.kad_query_to_peer.get(&id).cloned() {
                            debug!("Kademlia search completed for peer {}", target_peer_id);
                            
                            // Look for the target peer in results
                            let mut found_addresses = Vec::new();
                            for peer_info in &peers {
                                if peer_info.peer_id == target_peer_id {
                                    found_addresses.extend(peer_info.addrs.clone());
                                }
                            }
                            
                            // Complete the search
                            self.complete_search_for_peer(target_peer_id, found_addresses).await;
                            
                            // Also emit discovery events for all found peers
                            for peer_info in peers {
                                if !peer_info.addrs.is_empty() {
                                    events.push(XRoutesEvent::KadPeerDiscovered {
                                        peer_id: peer_info.peer_id,
                                        addresses: peer_info.addrs,
                                    });
                                }
                            }
                        }
                    }
                    kad::QueryResult::GetClosestPeers(Err(e)) => {
                        // Handle failed search
                        if let Some(target_peer_id) = self.kad_query_to_peer.get(&id).cloned() {
                            warn!("Kademlia search failed for peer {}: {:?}", target_peer_id, e);
                            self.fail_search_for_peer(target_peer_id, format!("DHT search failed: {:?}", e)).await;
                        }
                    }
                    _ => {}
                }
            }

            _ => {
                debug!("Other Kad event: {event:?}");
            }
        }

        events
    }

    /// Complete search for a specific peer with found addresses
    async fn complete_search_for_peer(&mut self, peer_id: PeerId, addresses: Vec<Multiaddr>) {
        if let Some(mut search_state) = self.active_searches.remove(&peer_id) {
            info!("Completing search for peer {} with {} addresses, {} waiters", 
                  peer_id, addresses.len(), search_state.waiters.len());
            
            // Remove query mapping
            if let Some(query_id) = search_state.query_id {
                self.kad_query_to_peer.remove(&query_id);
            }
            
            // Check for timed out waiters and send results to active ones
            let mut active_waiters = Vec::new();
            let mut timed_out_waiters = Vec::new();
            
            for waiter in search_state.waiters {
                if waiter.timeout_secs > 0 && waiter.start_time.elapsed().as_secs() >= waiter.timeout_secs as u64 {
                    // This waiter has timed out
                    timed_out_waiters.push(waiter);
                } else {
                    // This waiter is still active
                    active_waiters.push(waiter);
                }
            }
            
            // Send timeout errors to timed out waiters
            for waiter in timed_out_waiters {
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                let _ = waiter.response_channel.send(Err("Search timeout".to_string()));
            }
            
            // Send results to active waiters
            for waiter in active_waiters {
                // Cancel timeout if it exists
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                
                // Send addresses to waiter
                let _ = waiter.response_channel.send(Ok(addresses.clone()));
            }
        }
    }

    /// Fail search for a specific peer
    async fn fail_search_for_peer(&mut self, peer_id: PeerId, error: String) {
        if let Some(search_state) = self.active_searches.remove(&peer_id) {
            warn!("Failing search for peer {}: {}", peer_id, error);
            
            // Remove query mapping
            if let Some(query_id) = search_state.query_id {
                self.kad_query_to_peer.remove(&query_id);
            }
            
            // Send error to all active waiters
            for waiter in search_state.waiters {
                // Cancel timeout if it exists
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                
                // Send error to waiter
                let _ = waiter.response_channel.send(Err(error.clone()));
            }
        }
    }

    /// Set XRoute role
    async fn set_role(
        &mut self,
        new_role: XRouteRole,
        _swarm: &mut Swarm<NodeBehaviour>,
    ) -> Result<(), String> {
        let old_role = self.current_role.clone();
        
        if old_role == new_role {
            return Ok(());
        }

        self.current_role = new_role.clone();
        
        info!("XRoute role changed from {:?} to {:?}", old_role, new_role);
        Ok(())
    }

    /// Get current role
    pub fn current_role(&self) -> &XRouteRole {
        &self.current_role
    }

    /// Check if mDNS is enabled
    pub fn is_mdns_enabled(&self) -> bool {
        self.mdns_enabled
    }

    /// Check if Kademlia is enabled
    pub fn is_kad_enabled(&self) -> bool {
        self.kad_enabled
    }
    
    /// Handle bootstrap connection
    async fn handle_bootstrap_connection(
        &mut self,
        addr: libp2p::Multiaddr,
        timeout_secs: Option<u64>,
        swarm: &mut Swarm<NodeBehaviour>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        use tokio::time::{timeout, Duration};
        
        let timeout_duration = Duration::from_secs(timeout_secs.unwrap_or(30));
        
        timeout(timeout_duration, async {
            // Extract peer_id from multiaddr
            let peer_id = extract_peer_id_from_addr(&addr)
                .ok_or_else(|| BootstrapError::InvalidAddress("Address must contain PeerId".to_string()))?;
            
            // Establish connection
            swarm.dial(addr.clone())
                .map_err(|e| BootstrapError::ConnectionFailed(format!("Dial failed: {}", e)))?;
            
            // Wait for connection to establish
            tokio::time::sleep(Duration::from_millis(1000)).await;
            
            // For now, assume it's a server if connection succeeded
            let role = XRouteRole::Server;
            
            // Verify it's actually a server
            if role != XRouteRole::Server {
                return Err(BootstrapError::NotABootstrapServer(role));
            }
            
            // Add to bootstrap nodes list
            self.bootstrap_nodes.push((peer_id, addr));
            
            Ok(BootstrapNodeInfo {
                peer_id,
                role,
                protocols: vec!["kad".to_string()],
                agent_version: "unknown".to_string(),
            })
        }).await
        .map_err(|_| BootstrapError::ConnectionTimeout)?
    }
}

// Helper function to extract peer ID from multiaddr
fn extract_peer_id_from_addr(addr: &libp2p::Multiaddr) -> Option<libp2p::PeerId> {
    use libp2p::multiaddr::Protocol;
    
    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}
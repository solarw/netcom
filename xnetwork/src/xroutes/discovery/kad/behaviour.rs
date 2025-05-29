// ./xroutes/discovery/kad/behaviour.rs

use libp2p::{
    Multiaddr, PeerId, identity, kad,
    swarm::{
        NetworkBehaviour, THandlerInEvent, ToSwarm, behaviour::toggle::Toggle, derive_prelude::*,
    },
};
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::task::{Context, Poll};
use std::time::Duration;
use tracing::{debug, info, warn};

use super::super::events::{DiscoveryEvent, DiscoverySource};
use super::commands::{KadCommand, SearchHandlerStats, BootstrapNodeInfo, BootstrapError};
use super::events::KadEvent;
use super::search_handler::{SearchHandler, SearchState, SearchWaiter};
use crate::xroutes::xroute::XRouteRole;

pub struct KadBehaviour {
    pub kad: Toggle<kad::Behaviour<kad::store::MemoryStore>>,
    pending_events: VecDeque<DiscoveryEvent>,
    key: identity::Keypair,
    server_mode: bool,
    search_handler: SearchHandler,
}

impl KadBehaviour {
    pub fn new(
        key: &identity::Keypair,
        enable_kad: bool,
        server_mode: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let kad = if enable_kad {
            let kad_behaviour = Self::create_kad_behaviour(key, server_mode);
            Toggle::from(Some(kad_behaviour))
        } else {
            Toggle::from(None)
        };

        Ok(KadBehaviour {
            kad,
            pending_events: VecDeque::new(),
            key: key.clone(),
            server_mode,
            search_handler: SearchHandler::new(),
        })
    }

    /// Create Kademlia behaviour with proper configuration
    fn create_kad_behaviour(
        key: &identity::Keypair,
        server_mode: bool,
    ) -> kad::Behaviour<kad::store::MemoryStore> {
        // Configure Kademlia
        let mut kad_config = kad::Config::default();

        if server_mode {
            // Server mode settings - more aggressive for bootstrap nodes
            kad_config.set_parallelism(NonZeroUsize::new(5).unwrap());
            kad_config.set_query_timeout(Duration::from_secs(20));
            kad_config.disjoint_query_paths(true);
            kad_config.set_kbucket_inserts(kad::BucketInserts::OnConnected);
            kad_config.set_replication_factor(NonZeroUsize::new(5).unwrap());
        } else {
            // Client mode settings
            kad_config.set_parallelism(NonZeroUsize::new(3).unwrap());
            kad_config.set_query_timeout(Duration::from_secs(30));
        }

        kad_config.set_kbucket_inserts(kad::BucketInserts::Manual);
        kad_config.set_periodic_bootstrap_interval(None);

        // Create Kademlia behaviour
        let kad_store = kad::store::MemoryStore::new(key.public().to_peer_id());
        let mut kad_behaviour = kad::Behaviour::with_config(
            key.public().to_peer_id(),
            kad_store,
            kad_config,
        );

        if server_mode {
            kad_behaviour.set_mode(Some(kad::Mode::Server));
        } else {
            kad_behaviour.set_mode(Some(kad::Mode::Client));
        }

        kad_behaviour
    }

    /// Enable Kademlia
    pub fn enable_kad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.kad.is_enabled() {
            return Ok(());
        }

        let kad_behaviour = Self::create_kad_behaviour(&self.key, self.server_mode);
        self.kad = Toggle::from(Some(kad_behaviour));
        info!("Kademlia enabled in {} mode", if self.server_mode { "server" } else { "client" });
        Ok(())
    }

    /// Disable Kademlia
    pub fn disable_kad(&mut self) {
        self.kad = Toggle::from(None);
        info!("Kademlia disabled");
    }

    /// Check if Kademlia is enabled
    pub fn is_kad_enabled(&self) -> bool {
        self.kad.is_enabled()
    }

    /// Bootstrap Kademlia DHT
    pub fn bootstrap(&mut self) -> Result<kad::QueryId, libp2p::kad::NoKnownPeers> {
        if let Some(kad) = self.kad.as_mut() {
            debug!("Starting Kademlia bootstrap");
            kad.bootstrap()
        } else {
            Err(libp2p::kad::NoKnownPeers())
        }
    }

    /// Get known peers from Kademlia routing table
    pub fn get_known_peers(&mut self) -> Vec<(PeerId, Vec<Multiaddr>)> {
        if let Some(kad) = self.kad.as_mut() {
            let mut peers = Vec::new();

            for kbucket in kad.kbuckets() {
                for entry in kbucket.iter() {
                    let addresses = entry.node.value.clone().into_vec();
                    let peer_id = entry.node.key.into_preimage();
                    if !addresses.is_empty() {
                        peers.push((peer_id, addresses));
                    }
                }
            }

            peers
        } else {
            Vec::new()
        }
    }

    /// Get addresses for a specific peer from Kademlia
    pub fn get_peer_addresses(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        if let Some(kad) = self.kad.as_mut() {
            let mut addresses = Vec::new();

            for bucket in kad.kbuckets() {
                for entry in bucket.iter() {
                    if entry.node.key.preimage() == peer_id {
                        addresses = entry.node.value.clone().into_vec();
                        break;
                    }
                }
                if !addresses.is_empty() {
                    break;
                }
            }

            addresses
        } else {
            Vec::new()
        }
    }

    /// Initiate a search for peer addresses (returns QueryId for tracking)
    pub fn find_peer(&mut self, peer_id: PeerId) -> Option<kad::QueryId> {
        if let Some(kad) = self.kad.as_mut() {
            debug!("Starting peer search for {}", peer_id);
            Some(kad.get_closest_peers(peer_id))
        } else {
            warn!("Cannot start peer search - Kademlia not enabled");
            None
        }
    }

    /// Add address to Kademlia routing table
    pub fn add_address(&mut self, peer_id: &PeerId, addr: Multiaddr) {
        if let Some(kad) = self.kad.as_mut() {
            kad.add_address(peer_id, addr.clone());
            debug!("Added address {} for peer {}", addr, peer_id);
        }
    }

    /// Remove address from Kademlia routing table
    pub fn remove_address(&mut self, peer_id: &PeerId, addr: &Multiaddr) {
        if let Some(kad) = self.kad.as_mut() {
            kad.remove_address(peer_id, addr);
            debug!("Removed address {} for peer {}", addr, peer_id);
        }
    }

    /// Get the current Kademlia mode
    pub fn kad_mode(&self) -> Option<kad::Mode> {
        if self.server_mode {
            Some(kad::Mode::Server)
        } else if self.kad.is_enabled() {
            Some(kad::Mode::Client)
        } else {
            None
        }
    }

    /// Set Kademlia mode
    pub fn set_kad_mode(&mut self, mode: kad::Mode) {
        if let Some(kad) = self.kad.as_mut() {
            kad.set_mode(Some(mode));
            self.server_mode = matches!(mode, kad::Mode::Server);
            info!("Kademlia mode set to {:?}", mode);
        }
    }

    /// Get statistics about the Kademlia routing table
    pub fn kad_stats(&mut self) -> super::commands::KadStats {
        if let Some(kad) = self.kad.as_mut() {
            let mut total_peers = 0;
            let mut buckets_with_peers = 0;

            for bucket in kad.kbuckets() {
                let bucket_size = bucket.iter().count();
                if bucket_size > 0 {
                    buckets_with_peers += 1;
                    total_peers += bucket_size;
                }
            }

            super::commands::KadStats {
                total_peers,
                active_buckets: buckets_with_peers,
                total_buckets: kad.kbuckets().count(),
            }
        } else {
            super::commands::KadStats {
                total_peers: 0,
                active_buckets: 0,
                total_buckets: 0,
            }
        }
    }

    /// Check if a specific peer is in the routing table
    pub fn has_peer(&mut self, peer_id: &PeerId) -> bool {
        if let Some(kad) = self.kad.as_mut() {
            for bucket in kad.kbuckets() {
                for entry in bucket.iter() {
                    if entry.node.key.preimage() == peer_id {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Handle advanced search request (internal method)
    fn handle_advanced_search_internal(
        &mut self,
        peer_id: PeerId,
        timeout_secs: i32,
        response: tokio::sync::oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    ) -> Vec<KadEvent> {
        // Step 1: Check local addresses first
        let local_addresses = self.get_peer_addresses(&peer_id);
        
        // Step 2: If timeout is 0 or we found addresses locally, return immediately
        if timeout_secs == 0 || !local_addresses.is_empty() {
            debug!("Returning {} local addresses for peer {}", local_addresses.len(), peer_id);
            let _ = response.send(Ok(local_addresses));
            return Vec::new();
        }
        
        // Step 3: If Kademlia is not enabled, return error
        if !self.is_kad_enabled() {
            let _ = response.send(Err("Kademlia not enabled, cannot perform DHT search".to_string()));
            return Vec::new();
        }
        
        // Step 4: Create search waiter and initiate DHT search
        let waiter_id = self.search_handler.next_waiter_id;
        self.search_handler.next_waiter_id += 1;
        
        let search_id = self.search_handler.next_search_id;
        self.search_handler.next_search_id += 1;
        
        // Create timeout handle if needed
        let timeout_handle = if timeout_secs > 0 {
            let timeout_duration = Duration::from_secs(timeout_secs as u64);
            let handle = tokio::spawn(async move {
                tokio::time::sleep(timeout_duration).await;
                debug!("Search timeout for peer {} (waiter {})", peer_id, waiter_id);
            });
            Some(handle.abort_handle())
        } else {
            None
        };
        
        // Create search waiter
        let waiter = SearchWaiter {
            response_channel: response,
            timeout_handle,
            start_time: std::time::Instant::now(),
            timeout_secs,
            waiter_id,
        };
        
        // Try to initiate DHT search
        let query_id = self.find_peer(peer_id);
        
        // Create or update search state
        if let Some(search_state) = self.search_handler.active_searches.get_mut(&peer_id) {
            debug!("Adding waiter to existing search for peer {}", peer_id);
            search_state.waiters.push(waiter);
            Vec::new()
        } else {
            debug!("Creating new search for peer {}", peer_id);
            
            let search_state = SearchState {
                query_id,
                waiters: vec![waiter],
                peer_id,
                created_at: std::time::Instant::now(),
                search_id,
            };
            
            // Map query ID to peer ID for later lookup
            if let Some(qid) = query_id {
                self.search_handler.kad_query_to_peer.insert(qid, peer_id);
            }
            
            self.search_handler.active_searches.insert(peer_id, search_state);
            
            vec![KadEvent::SearchStarted {
                peer_id,
                search_id,
                timeout_secs,
            }]
        }
    }

    // ==========================================
    // ADVANCED SEARCH API (Convenience Methods)
    // ==========================================

    /// Find peer addresses with advanced timeout control (convenience method)
    /// 
    /// Returns a oneshot receiver that will contain the result
    pub fn find_peer_addresses_advanced(
        &mut self,
        peer_id: PeerId,
        timeout_secs: i32,
    ) -> (tokio::sync::oneshot::Receiver<Result<Vec<Multiaddr>, String>>, Vec<KadEvent>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let events = self.handle_advanced_search_internal(peer_id, timeout_secs, tx);
        (rx, events)
    }

    /// Cancel search for specific peer (convenience method)
    pub fn cancel_peer_search(&mut self, peer_id: &PeerId) -> (Result<(), String>, Vec<KadEvent>) {
        self.search_handler.cancel_peer_search_sync(peer_id)
    }

    /// Cancel all active searches (convenience method)
    pub fn cancel_all_searches(&mut self, reason: &str) -> Vec<KadEvent> {
        self.search_handler.cancel_all_searches_sync(reason)
    }

    /// Get information about active searches (convenience method)
    pub fn get_active_searches(&self) -> Vec<(PeerId, usize, Duration)> {
        self.search_handler.get_active_searches_info()
    }

    /// Get search handler statistics (convenience method)
    pub fn get_search_stats(&self) -> SearchHandlerStats {
        self.search_handler.get_statistics()
    }

    /// Check if there are active searches for a peer (convenience method)
    pub fn has_active_search(&self, peer_id: &PeerId) -> bool {
        self.search_handler.has_active_search(peer_id)
    }

    /// Update peer cache with addresses (convenience method)
    pub fn update_peer_cache(&mut self, peer_id: PeerId, addresses: Vec<Multiaddr>) {
        self.search_handler.update_peer_cache(peer_id, addresses);
    }

    /// Get cached addresses for a peer (convenience method)
    pub fn get_cached_addresses(&self, peer_id: &PeerId) -> Option<&Vec<Multiaddr>> {
        self.search_handler.get_cached_addresses(peer_id)
    }

    /// Clear the peer cache (convenience method)
    pub fn clear_cache(&mut self) {
        self.search_handler.clear_cache();
    }

    /// Cleanup timed out waiters (convenience method)
    pub fn cleanup_timed_out_waiters(&mut self) -> Vec<KadEvent> {
        self.search_handler.cleanup_timed_out_waiters()
    }

    /// Handle identify events for address management
    pub fn handle_identify_event(&mut self, event: &libp2p::identify::Event) {
        match event {
            libp2p::identify::Event::Received { peer_id, info, .. } => {
                // Add peer's listening addresses to Kademlia routing table
                for addr in &info.listen_addrs {
                    self.add_address(peer_id, addr.clone());
                    debug!("Added address from identify: {} -> {}", peer_id, addr);
                }
            }
            _ => {
                // Handle other identify events if needed
            }
        }
    }

    // ==========================================
    // COMPATIBILITY API (Legacy Methods)
    // ==========================================

    /// Find peer addresses with timeout in seconds (convenience method)
    /// 
    /// This is a convenience wrapper that returns the result directly
    pub async fn find_peer_addresses_with_timeout(
        &mut self,
        peer_id: PeerId,
        timeout_secs: u32,
    ) -> Result<Vec<Multiaddr>, String> {
        let (rx, _events) = self.find_peer_addresses_advanced(peer_id, timeout_secs as i32);
        rx.await.map_err(|e| format!("Internal error: {}", e))?
    }

    /// Find peer addresses from local tables only (convenience method)
    /// 
    /// This method only checks local routing table and cache, no DHT search
    pub fn find_peer_addresses_local_only(
        &mut self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, String> {
        let mut addresses = Vec::new();

        // Get from Kademlia routing table
        let kad_addresses = self.get_peer_addresses(&peer_id);
        addresses.extend(kad_addresses);

        // Get from search handler cache
        if let Some(cached_addresses) = self.search_handler.get_cached_addresses(&peer_id) {
            addresses.extend(cached_addresses.clone());
        }

        // Deduplicate addresses
        addresses.sort();
        addresses.dedup();

        Ok(addresses)
    }

    /// Find peer addresses with infinite timeout (convenience method)
    /// 
    /// This method will search until explicitly cancelled or peer is found
    pub async fn find_peer_addresses_infinite(
        &mut self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, String> {
        let (rx, _events) = self.find_peer_addresses_advanced(peer_id, -1);
        rx.await.map_err(|e| format!("Internal error: {}", e))?
    }

    /// Connect to a bootstrap node and verify it's in server mode
    /// 
    /// This method attempts to connect to a bootstrap node and verify its capabilities
    pub async fn connect_to_bootstrap_node(
        &mut self,
        addr: Multiaddr,
        timeout_secs: Option<u64>,
    ) -> Result<BootstrapNodeInfo, BootstrapError> {
        use tokio::time::{Duration, timeout};

        let timeout_duration = Duration::from_secs(timeout_secs.unwrap_or(30));

        timeout(timeout_duration, async {
            // Extract peer_id from multiaddr
            let peer_id = self.extract_peer_id_from_addr(&addr)
                .ok_or_else(|| BootstrapError::InvalidAddress(
                    "Address must contain PeerId".to_string()
                ))?;

            // Add the address to our routing table
            self.add_address(&peer_id, addr.clone());

            // Try to initiate a DHT query to test connectivity
            if let Some(_query_id) = self.find_peer(peer_id) {
                debug!("Initiated DHT query to test bootstrap node {}", peer_id);
            }

            // Wait a bit for potential routing table updates
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Check if the peer is now in our routing table
            let addresses = self.get_peer_addresses(&peer_id);
            if addresses.is_empty() {
                return Err(BootstrapError::ConnectionFailed(
                    "Bootstrap node did not respond to DHT queries".to_string()
                ));
            }

            // For now, assume it's a server if we can reach it via DHT
            // In a real implementation, you might want to check specific protocols
            let role = XRouteRole::Server;

            // Verify it's actually a server by checking if it responds to DHT queries
            if role != XRouteRole::Server {
                return Err(BootstrapError::NotABootstrapServer(role));
            }

            Ok(BootstrapNodeInfo {
                peer_id,
                role,
                protocols: vec!["kad".to_string()],
                agent_version: "unknown".to_string(),
            })
        })
        .await
        .map_err(|_| BootstrapError::ConnectionTimeout)?
    }

    /// Extract peer ID from multiaddr (helper method)
    fn extract_peer_id_from_addr(&self, addr: &Multiaddr) -> Option<PeerId> {
        use libp2p::multiaddr::Protocol;

        for protocol in addr.iter() {
            if let Protocol::P2p(peer_id) = protocol {
                return Some(peer_id);
            }
        }
        None
    }

    /// Handle Kademlia events and convert to discovery events
    fn handle_kad_event(&mut self, event: kad::Event) -> Vec<KadEvent> {
        let mut events = Vec::new();

        match event {
            kad::Event::RoutingUpdated { peer, addresses, .. } => {
                debug!("Kademlia routing updated for peer: {}", peer);
                
                let addr_vec: Vec<Multiaddr> = addresses.iter().cloned().collect();
                if !addr_vec.is_empty() {
                    // Update search handler cache and handle any active searches
                    let search_events = self.search_handler.handle_routing_update_sync(peer, addr_vec.clone());
                    events.extend(search_events);

                    // Also emit discovery event
                    self.pending_events.push_back(DiscoveryEvent::PeerDiscovered {
                        peer_id: peer,
                        addresses: addr_vec.clone(),
                        source: DiscoverySource::Kad,
                    });

                    events.push(KadEvent::RoutingUpdated {
                        peer_id: peer,
                        addresses: addr_vec,
                    });
                }
            }

            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                match result {
                    kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { peers, .. })) => {
                        debug!("Kademlia query {} completed with {} peers", id, peers.len());
                        
                        // Convert to our format
                        let peer_results: Vec<(PeerId, Vec<Multiaddr>)> = peers.iter()
                            .map(|peer_info| (peer_info.peer_id, peer_info.addrs.clone()))
                            .collect();

                        // Notify search handler about query completion
                        let search_events = self.search_handler.handle_kad_query_completed_sync(id, Ok(peer_results.clone()));
                        events.extend(search_events);

                        // Group peers by peer_id and emit discovery events
                        for (peer_id, addresses) in &peer_results {
                            if !addresses.is_empty() {
                                self.pending_events.push_back(DiscoveryEvent::PeerDiscovered {
                                    peer_id: *peer_id,
                                    addresses: addresses.clone(),
                                    source: DiscoverySource::Kad,
                                });
                            }
                        }

                        events.push(KadEvent::QueryCompleted {
                            query_id: id,
                            result: super::events::QueryResult::FoundPeers(peer_results),
                        });
                    }
                    
                    kad::QueryResult::GetClosestPeers(Err(e)) => {
                        warn!("Kademlia query {} failed: {:?}", id, e);
                        
                        // Notify search handler about query failure
                        let search_events = self.search_handler.handle_kad_query_completed_sync(id, Err(format!("{:?}", e)));
                        events.extend(search_events); //handle synchronously  
                        // let search_events = self.search_handler.handle_kad_query_completed_sync(id, Err(format!("{:?}", e)));
                        // events.extend(search_events);

                        events.push(KadEvent::QueryFailed {
                            query_id: id,
                            error: format!("{:?}", e),
                        });
                    }
                    
                    _ => {
                        debug!("Other Kademlia query result: {:?}", result);
                        events.push(KadEvent::QueryCompleted {
                            query_id: id,
                            result: super::events::QueryResult::Other(format!("{:?}", result)),
                        });
                    }
                }
            }

            kad::Event::InboundRequest { .. } => {
                // Handle inbound DHT requests - usually we don't need to emit events for these
                debug!("Received inbound Kademlia request");
            }

            _ => {
                debug!("Other Kademlia event: {:?}", event);
            }
        }

        events
    }

    /// Handle commands
    pub fn handle_command(&mut self, cmd: KadCommand) -> Vec<KadEvent> {
        let mut events = Vec::new();

        match cmd {
            // Basic Kademlia commands
            KadCommand::EnableKad => {
                self.enable_kad();
                events.push(KadEvent::KadEnabled);
            }

            KadCommand::DisableKad => {
                self.disable_kad();
                events.push(KadEvent::KadDisabled);
            }

            KadCommand::Bootstrap => {
                if let Err(e) = self.bootstrap() {
                    warn!("Failed to bootstrap Kademlia: {}", e);
                    events.push(KadEvent::BootstrapFailed {
                        error: format!("{:?}", e),
                    });
                }
            }

            KadCommand::AddAddress { peer_id, addr } => {
                self.add_address(&peer_id, addr.clone());
                events.push(KadEvent::AddressAdded { peer_id, addr });
            }

            KadCommand::RemoveAddress { peer_id, addr } => {
                self.remove_address(&peer_id, &addr);
                events.push(KadEvent::AddressRemoved { peer_id, addr });
            }

            KadCommand::FindPeer { peer_id } => {
                if let None = self.find_peer(peer_id) {
                    warn!("Failed to start peer search for {}", peer_id);
                }
            }

            KadCommand::SetMode { mode } => {
                let old_mode = self.kad_mode();
                self.set_kad_mode(mode);
                events.push(KadEvent::ModeChanged {
                    old_mode,
                    new_mode: mode,
                });
            }

            // Query commands with responses
            KadCommand::GetKnownPeers { response } => {
                let peers = self.get_known_peers();
                let _ = response.send(peers);
            }

            KadCommand::GetPeerAddresses { peer_id, response } => {
                let addresses = self.get_peer_addresses(&peer_id);
                let _ = response.send(addresses);
            }

            KadCommand::GetKadStats { response } => {
                let stats = self.kad_stats();
                let _ = response.send(stats);
            }

            // Advanced search commands - теперь реализованы
            KadCommand::FindPeerAddressesAdvanced {
                peer_id,
                timeout_secs,
                response,
            } => {
                let search_events = self.handle_advanced_search_internal(peer_id, timeout_secs, response);
                events.extend(search_events);
            }

            KadCommand::CancelPeerSearch { peer_id, response } => {
                let (result, search_events) = self.search_handler.cancel_peer_search_sync(&peer_id);
                events.extend(search_events);
                let _ = response.send(result);
            }

            KadCommand::CancelAllSearches { reason, response } => {
                let cancel_events = self.search_handler.cancel_all_searches_sync(&reason);
                events.extend(cancel_events);
                let _ = response.send(());
            }

            KadCommand::GetActiveSearches { response } => {
                let searches = self.search_handler.get_active_searches_info();
                let _ = response.send(searches);
            }

            KadCommand::GetSearchStats { response } => {
                let stats = self.search_handler.get_statistics();
                let _ = response.send(stats);
            }

            KadCommand::CleanupTimedOutWaiters => {
                let cleanup_events = self.search_handler.cleanup_timed_out_waiters();
                events.extend(cleanup_events);
            }

            // Convenience/Compatibility commands
            KadCommand::FindPeerAddressesWithTimeout { peer_id, timeout_secs, response } => {
                // Spawn async task to handle this
                let (tx, rx) = tokio::sync::oneshot::channel();
                let search_events = self.handle_advanced_search_internal(peer_id, timeout_secs as i32, tx);
                events.extend(search_events);
                
                // Forward the result
                tokio::spawn(async move {
                    match rx.await {
                        Ok(result) => { let _ = response.send(result); },
                        Err(_) => { let _ = response.send(Err("Internal error".to_string())); },
                    }
                });
            }

            KadCommand::FindPeerAddressesLocalOnly { peer_id, response } => {
                let result = self.find_peer_addresses_local_only(peer_id);
                let _ = response.send(result);
            }

            KadCommand::FindPeerAddressesInfinite { peer_id, response } => {
                // Spawn async task to handle this
                let (tx, rx) = tokio::sync::oneshot::channel();
                let search_events = self.handle_advanced_search_internal(peer_id, -1, tx);
                events.extend(search_events);
                
                // Forward the result
                tokio::spawn(async move {
                    match rx.await {
                        Ok(result) => { let _ = response.send(result); },
                        Err(_) => { let _ = response.send(Err("Internal error".to_string())); },
                    }
                });
            }

            KadCommand::ConnectToBootstrapNode { addr, timeout_secs, response } => {
                // This needs to be handled differently since it requires async
                // For now, return an error indicating async context is needed
                let _ = response.send(Err(BootstrapError::ConnectionFailed(
                    "Bootstrap connection requires async context - use convenience method instead".to_string()
                )));
            }
        }

        events
    }
}

impl NetworkBehaviour for KadBehaviour {
    type ConnectionHandler = <Toggle<kad::Behaviour<kad::store::MemoryStore>> as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = DiscoveryEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.kad.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.kad.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        self.kad.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.kad.on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Return pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        // Poll Kademlia for events
        match self.kad.poll(cx) {
            Poll::Ready(ToSwarm::GenerateEvent(kad_event)) => {
                // Handle Kademlia event and convert to discovery events
                self.handle_kad_event(kad_event);
            }
            Poll::Ready(other_event) => {
                // Pass through other event types unchanged
                return Poll::Ready(other_event.map_out(|_| unreachable!()));
            }
            Poll::Pending => {}
        }

        // Check for pending events generated by kad event handling
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        Poll::Pending
    }
}

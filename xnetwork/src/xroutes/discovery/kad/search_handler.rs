// ./xroutes/discovery/kad/search_handler.rs

use libp2p::{Multiaddr, PeerId, kad};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{debug, info, warn};

use super::commands::SearchHandlerStats;
use super::events::KadEvent;

// Structure to track individual search waiters with their own timeouts
#[derive(Debug)]
pub struct SearchWaiter {
    pub response_channel: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
    pub timeout_handle: Option<tokio::task::AbortHandle>,
    pub start_time: Instant,
    pub timeout_secs: i32,
    pub waiter_id: u64,
}

// Structure to track search state for a specific peer
#[derive(Debug)]
pub struct SearchState {
    pub query_id: Option<kad::QueryId>,
    pub waiters: Vec<SearchWaiter>,
    pub peer_id: PeerId,
    pub created_at: Instant,
    pub search_id: u64,
}

/// Advanced search handler for sophisticated peer discovery with timeout management
pub struct SearchHandler {
    /// Active searches tracking
    pub active_searches: HashMap<PeerId, SearchState>,
    /// Counter for unique waiter IDs
    pub next_waiter_id: u64,
    /// Counter for unique search IDs
    pub next_search_id: u64,
    /// Map query IDs to peer IDs for tracking
    pub kad_query_to_peer: HashMap<kad::QueryId, PeerId>,
    /// Cache for discovered peers from various sources
    discovered_peers_cache: HashMap<PeerId, Vec<Multiaddr>>,
}

impl SearchHandler {
    pub fn new() -> Self {
        Self {
            active_searches: HashMap::new(),
            next_waiter_id: 1,
            next_search_id: 1,
            kad_query_to_peer: HashMap::new(),
            discovered_peers_cache: HashMap::new(),
        }
    }

    /// Handle advanced peer address finding with individual timeouts (synchronous version)
    /// 
    /// # Arguments
    /// * `peer_id` - The peer to find addresses for
    /// * `timeout_secs` - Timeout behavior:
    ///   - `0` - Check local tables only, no DHT search
    ///   - `>0` - Search with specified timeout in seconds
    ///   - `-1` - Infinite search until explicitly cancelled
    /// * `response` - Channel to send results
    /// * `get_local_addresses` - Closure to get local addresses from KadBehaviour
    /// * `initiate_search` - Closure to initiate DHT search
    pub fn handle_find_peer_addresses_advanced_sync<F1, F2>(
        &mut self,
        peer_id: PeerId,
        timeout_secs: i32,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
        mut get_local_addresses: F1,
        mut initiate_search: F2,
    ) -> Vec<KadEvent>
    where
        F1: FnMut(&PeerId) -> Vec<Multiaddr>,
        F2: FnMut(&PeerId) -> Option<kad::QueryId>,
    {
        let mut events = Vec::new();

        debug!(
            "Finding addresses for peer {} with timeout {}",
            peer_id, timeout_secs
        );

        // Step 1: Check local tables first
        let local_addresses = self.get_local_peer_addresses(&peer_id, &mut get_local_addresses);

        // Step 2: If timeout is 0 or we found addresses locally, return immediately
        if timeout_secs == 0 || !local_addresses.is_empty() {
            debug!(
                "Returning {} local addresses for peer {}",
                local_addresses.len(),
                peer_id
            );
            let _ = response.send(Ok(local_addresses));
            return events;
        }

        // Step 3: Create search waiter synchronously
        let waiter = self.create_search_waiter_sync(response, timeout_secs, peer_id);

        // Step 4: Add to existing search or create new one
        if let Some(search_state) = self.active_searches.get_mut(&peer_id) {
            debug!("Adding waiter to existing search for peer {}", peer_id);
            search_state.waiters.push(waiter);
        } else {
            debug!("Creating new search for peer {}", peer_id);
            let search_id = self.next_search_id;
            self.next_search_id += 1;

            let query_id = initiate_search(&peer_id);

            let search_state = SearchState {
                query_id,
                waiters: vec![waiter],
                peer_id,
                created_at: Instant::now(),
                search_id,
            };

            // Map query ID to peer ID for later lookup
            if let Some(qid) = query_id {
                self.kad_query_to_peer.insert(qid, peer_id);
            }

            self.active_searches.insert(peer_id, search_state);

            // Emit search started event
            events.push(KadEvent::SearchStarted {
                peer_id,
                search_id,
                timeout_secs,
            });
        }

        events
    }

    /// Create a search waiter with timeout handling (synchronous version)
    fn create_search_waiter_sync(
        &mut self,
        response: oneshot::Sender<Result<Vec<Multiaddr>, String>>,
        timeout_secs: i32,
        peer_id: PeerId,
    ) -> SearchWaiter {
        let waiter_id = self.next_waiter_id;
        self.next_waiter_id += 1;

        let timeout_handle = if timeout_secs > 0 {
            // Create timeout task
            let timeout_duration = Duration::from_secs(timeout_secs as u64);
            let handle = tokio::spawn(async move {
                tokio::time::sleep(timeout_duration).await;
                debug!("Search timeout for peer {} (waiter {})", peer_id, waiter_id);
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

    /// Get addresses from local tables and cache
    fn get_local_peer_addresses<F>(
        &self,
        peer_id: &PeerId,
        get_local_addresses: &mut F,
    ) -> Vec<Multiaddr>
    where
        F: FnMut(&PeerId) -> Vec<Multiaddr>,
    {
        let mut addresses = Vec::new();

        // Get from KadBehaviour routing table
        let kad_addresses = get_local_addresses(peer_id);
        addresses.extend(kad_addresses);

        // Get from local cache
        if let Some(cached_addresses) = self.discovered_peers_cache.get(peer_id) {
            addresses.extend(cached_addresses.clone());
        }

        // Deduplicate addresses
        addresses.sort();
        addresses.dedup();

        debug!(
            "Found {} local addresses for peer {}",
            addresses.len(),
            peer_id
        );
        addresses
    }

    /// Cancel search for specific peer (synchronous version)
    pub fn cancel_peer_search_sync(&mut self, peer_id: &PeerId) -> (Result<(), String>, Vec<KadEvent>) {
        let mut events = Vec::new();
        
        if let Some(search_state) = self.active_searches.remove(peer_id) {
            info!(
                "Cancelling search for peer {} with {} waiters",
                peer_id,
                search_state.waiters.len()
            );

            // Cancel all waiters
            for waiter in search_state.waiters {
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                let _ = waiter
                    .response_channel
                    .send(Err("Search cancelled".to_string()));
            }

            // Remove query ID mapping
            if let Some(query_id) = search_state.query_id {
                self.kad_query_to_peer.remove(&query_id);
            }

            events.push(KadEvent::SearchCancelled {
                peer_id: *peer_id,
                search_id: search_state.search_id,
                reason: "User requested cancellation".to_string(),
            });

            (Ok(()), events)
        } else {
            (Err(format!("No active search found for peer {}", peer_id)), events)
        }
    }

    /// Cancel all active searches (synchronous version)
    pub fn cancel_all_searches_sync(&mut self, reason: &str) -> Vec<KadEvent> {
        let peer_ids: Vec<PeerId> = self.active_searches.keys().cloned().collect();
        let count = peer_ids.len();

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

        vec![KadEvent::AllSearchesCancelled {
            count,
            reason: reason.to_string(),
        }]
    }

    /// Clean up timed out waiters for all active searches
    pub fn cleanup_timed_out_waiters(&mut self) -> Vec<KadEvent> {
        let mut events = Vec::new();
        let mut peers_to_remove = Vec::new();
        let mut total_cleaned = 0;

        for (peer_id, search_state) in self.active_searches.iter_mut() {
            let mut active_waiters = Vec::new();
            let mut timed_out_waiters = Vec::new();

            // Separate active and timed out waiters
            for waiter in search_state.waiters.drain(..) {
                if waiter.timeout_secs > 0
                    && waiter.start_time.elapsed().as_secs() >= waiter.timeout_secs as u64
                {
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
                let _ = waiter
                    .response_channel
                    .send(Err("Search timeout".to_string()));
                total_cleaned += 1;
            }

            // Emit timeout events if needed
            if !active_waiters.is_empty() && search_state.waiters.len() != active_waiters.len() {
                events.push(KadEvent::SearchTimedOut {
                    peer_id: *peer_id,
                    search_id: search_state.search_id,
                    duration: search_state.created_at.elapsed(),
                });
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
                debug!(
                    "Removed search for peer {} - all waiters timed out",
                    peer_id
                );
            }
        }

        if total_cleaned > 0 {
            info!("Cleaned up {} timed out search waiters", total_cleaned);
        }

        events
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

    /// Handle Kademlia query completion (synchronous version)
    pub fn handle_kad_query_completed_sync(
        &mut self,
        query_id: kad::QueryId,
        result: Result<Vec<(PeerId, Vec<Multiaddr>)>, String>,
    ) -> Vec<KadEvent> {
        let mut events = Vec::new();

        // Find which peer this query was for
        if let Some(target_peer_id) = self.kad_query_to_peer.get(&query_id).cloned() {
            match result {
                Ok(peer_results) => {
                    // Look for the target peer in results
                    let mut found_addresses = Vec::new();
                    for (peer_id, addresses) in &peer_results {
                        if *peer_id == target_peer_id {
                            found_addresses.extend(addresses.clone());
                        }
                        // Cache all discovered peers
                        self.discovered_peers_cache.insert(*peer_id, addresses.clone());
                    }

                    // Complete the search
                    let search_events = self.complete_search_for_peer_sync(target_peer_id, found_addresses);
                    events.extend(search_events);
                }
                Err(error) => {
                    // Handle failed search
                    warn!(
                        "Kademlia search failed for peer {}: {}",
                        target_peer_id, error
                    );
                    let search_events = self.fail_search_for_peer_sync(target_peer_id, error);
                    events.extend(search_events);
                }
            }
        }

        events
    }

    /// Complete search for a specific peer with found addresses (synchronous version)
    fn complete_search_for_peer_sync(&mut self, peer_id: PeerId, addresses: Vec<Multiaddr>) -> Vec<KadEvent> {
        let mut events = Vec::new();

        if let Some(mut search_state) = self.active_searches.remove(&peer_id) {
            let duration = search_state.created_at.elapsed();
            
            info!(
                "Completing search for peer {} with {} addresses, {} waiters",
                peer_id,
                addresses.len(),
                search_state.waiters.len()
            );

            // Remove query mapping
            if let Some(query_id) = search_state.query_id {
                self.kad_query_to_peer.remove(&query_id);
            }

            // Check for timed out waiters and send results to active ones
            let mut active_waiters = Vec::new();
            let mut timed_out_waiters = Vec::new();

            for waiter in search_state.waiters {
                if waiter.timeout_secs > 0
                    && waiter.start_time.elapsed().as_secs() >= waiter.timeout_secs as u64
                {
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
                let _ = waiter
                    .response_channel
                    .send(Err("Search timeout".to_string()));
            }

            // Send results to active waiters
            for waiter in active_waiters {
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                let _ = waiter.response_channel.send(Ok(addresses.clone()));
            }

            // Emit completion event
            events.push(KadEvent::SearchCompleted {
                peer_id,
                search_id: search_state.search_id,
                addresses,
                duration,
            });
        }

        events
    }

    /// Fail search for a specific peer (synchronous version)
    fn fail_search_for_peer_sync(&mut self, peer_id: PeerId, error: String) -> Vec<KadEvent> {
        let mut events = Vec::new();

        if let Some(search_state) = self.active_searches.remove(&peer_id) {
            let duration = search_state.created_at.elapsed();
            
            warn!("Failing search for peer {}: {}", peer_id, error);

            // Remove query mapping
            if let Some(query_id) = search_state.query_id {
                self.kad_query_to_peer.remove(&query_id);
            }

            // Send error to all active waiters
            for waiter in search_state.waiters {
                if let Some(handle) = waiter.timeout_handle {
                    handle.abort();
                }
                let _ = waiter.response_channel.send(Err(error.clone()));
            }

            // Emit failure event
            events.push(KadEvent::SearchFailed {
                peer_id,
                search_id: search_state.search_id,
                error,
                duration,
            });
        }

        events
    }

    /// Update peer cache with discovered addresses
    pub fn update_peer_cache(&mut self, peer_id: PeerId, addresses: Vec<Multiaddr>) {
        self.discovered_peers_cache.insert(peer_id, addresses);
    }

    /// Get cached addresses for a peer
    pub fn get_cached_addresses(&self, peer_id: &PeerId) -> Option<&Vec<Multiaddr>> {
        self.discovered_peers_cache.get(peer_id)
    }

    /// Clear the peer cache
    pub fn clear_cache(&mut self) {
        self.discovered_peers_cache.clear();
    }

    /// Get statistics about the search handler
    pub fn get_statistics(&self) -> SearchHandlerStats {
        SearchHandlerStats {
            active_searches: self.active_searches.len(),
            total_waiters: self.active_searches.values()
                .map(|s| s.waiters.len())
                .sum(),
            cached_peers: self.discovered_peers_cache.len(),
            next_waiter_id: self.next_waiter_id,
        }
    }

    /// Check if there are active searches for a peer
    pub fn has_active_search(&self, peer_id: &PeerId) -> bool {
        self.active_searches.contains_key(peer_id)
    }

    /// Get search info for a specific peer
    pub fn get_search_info(&self, peer_id: &PeerId) -> Option<(u64, usize, Duration)> {
        self.active_searches.get(peer_id).map(|search_state| {
            (
                search_state.search_id,
                search_state.waiters.len(),
                search_state.created_at.elapsed(),
            )
        })
    }

    /// Handle routing table updates from Kademlia (synchronous version)
    pub fn handle_routing_update_sync(&mut self, peer_id: PeerId, addresses: Vec<Multiaddr>) -> Vec<KadEvent> {
        let mut events = Vec::new();

        // Update cache
        self.update_peer_cache(peer_id, addresses.clone());

        // Check if we have active searches for this peer
        if self.has_active_search(&peer_id) && !addresses.is_empty() {
            // Complete the search with the new addresses
            let search_events = self.complete_search_for_peer_sync(peer_id, addresses);
            events.extend(search_events);
        }

        events
    }
}
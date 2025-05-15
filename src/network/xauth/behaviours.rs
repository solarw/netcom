use libp2p::{
    core::{Endpoint, Multiaddr},
    request_response::{self, ResponseChannel},
    swarm::{
        derive_prelude::*, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, ToSwarm,
    },
    PeerId, StreamProtocol,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use crate::network::xauth::definitions::DirectionalAuthState;

// Import the ProofOfRepresentation from the por module
use super::{
    connection_data::ConnectionData,
    definitions::{
        AuthDirection, AuthResult, CombinedAuthState, PendingVerification, PorAuthRequest,
        PorAuthResponse, AUTH_TIMEOUT, PROTOCOL_ID,
    },
    events::PorAuthEvent,
    por::por::ProofOfRepresentation,
};

// Define the behaviour
pub struct PorAuthBehaviour {
    // Using cbor codec for request-response
    pub request_response: request_response::cbor::Behaviour<PorAuthRequest, PorAuthResponse>,

    // Improved connection tracking - now using ConnectionId
    connections: HashMap<ConnectionId, ConnectionData>,

    // Lookup from PeerId to ConnectionId(s)
    peer_connections: HashMap<PeerId, HashSet<ConnectionId>>,

    // Events to be emitted
    pending_events: VecDeque<ToSwarm<PorAuthEvent, request_response::OutboundRequestId>>,

    // Authentication data (PoR)
    por: ProofOfRepresentation,

    // Additional metadata to send with auth request
    metadata: HashMap<String, String>,

    // Storage for pending PoR verifications using ConnectionId
    pending_verifications: HashMap<ConnectionId, PendingVerification>,
}

impl PorAuthBehaviour {
    pub fn new(por: ProofOfRepresentation) -> Self {
        let metadata = HashMap::new();
        Self::with_metadata(por, metadata)
    }

    pub fn with_metadata(por: ProofOfRepresentation, metadata: HashMap<String, String>) -> Self {
        Self {
            request_response: request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new(PROTOCOL_ID),
                    request_response::ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            ),
            connections: HashMap::new(),
            peer_connections: HashMap::new(),
            pending_events: VecDeque::new(),
            por,
            metadata,
            pending_verifications: HashMap::new(),
        }
    }

    // Update the PoR data used for authentication
    pub fn update_por(&mut self, por: ProofOfRepresentation) {
        self.por = por;
    }

    // Update the metadata sent with authentication
    pub fn update_metadata(&mut self, metadata: HashMap<String, String>) {
        self.metadata = metadata;
    }

    // Handle incoming authentication request
    fn handle_auth_request(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        request: PorAuthRequest,
        channel: ResponseChannel<PorAuthResponse>,
    ) {
        // Log the received authentication request
        println!(
            "Received auth request from {:?} on connection {:?}",
            peer_id, connection_id
        );

        if let Some(conn) = self.connections.get_mut(&connection_id) {
            conn.touch();

            // Get address for event
            let address = conn.address.clone();

            // Store the verification request with connection_id
            let verification = PendingVerification {
                peer_id,
                address: address.clone(),
                por: request.por.clone(),
                metadata: request.metadata.clone(),
                response_channel: channel, // канал сохраняется только здесь
                received_at: Instant::now(),
            };

            self.pending_verifications
                .insert(connection_id, verification);

            // Forward the PoR verification request to the application (без channel)
            self.pending_events
                .push_back(ToSwarm::GenerateEvent(PorAuthEvent::VerifyPorRequest {
                    peer_id,
                    connection_id,
                    address,
                    por: request.por,
                    metadata: request.metadata,
                }));
        }
    }

    // Process the authentication response from the remote peer
    fn handle_auth_response(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        response: PorAuthResponse,
    ) {
        println!(
            "Received auth response from {:?} on connection {:?}: {:?}",
            peer_id, connection_id, response.result
        );

        if let Some(conn) = self.connections.get_mut(&connection_id) {
            conn.touch();

            match response.result {
                AuthResult::Ok(metadata) => {
                    // Update connection state
                    conn.set_outbound_auth_success(metadata.clone());

                    // Generate outbound auth success event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::OutboundAuthSuccess {
                            peer_id,
                            connection_id,
                            address: conn.address.clone(),
                            metadata: metadata.clone(),
                        },
                    ));

                    // Check if we now have full mutual authentication
                    if matches!(
                        conn.get_combined_state(),
                        CombinedAuthState::FullyAuthenticated(_)
                    ) {
                        // Generate mutual auth success event
                        if let Some(metadata) = conn.get_metadata() {
                            self.pending_events.push_back(ToSwarm::GenerateEvent(
                                PorAuthEvent::MutualAuthSuccess {
                                    peer_id,
                                    connection_id,
                                    address: conn.address.clone(),
                                    metadata,
                                },
                            ));
                        }
                    }
                }
                AuthResult::Error(reason) => {
                    // Update state
                    conn.set_outbound_auth_failed(reason.clone());

                    // Generate inbound auth failure event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::InboundAuthFailure {
                            peer_id,
                            connection_id,
                            address: conn.address.clone(),
                            reason,
                        },
                    ));
                }
            }
        }
    }

    // Start outbound authentication process with a peer
    fn start_outbound_auth(&mut self, peer_id: PeerId, connection_id: ConnectionId) {
        if let Some(conn) = self.connections.get_mut(&connection_id) {
            // Update state
            conn.start_outbound_auth();

            println!(
                "Starting outbound auth with peer {:?} on connection {:?}",
                peer_id, connection_id
            );

            // Send authentication request
            let request = PorAuthRequest {
                por: self.por.clone(),
                metadata: self.metadata.clone(),
            };

            self.request_response.send_request(&peer_id, request);
        }
    }

    // Start inbound authentication waiting
    fn start_inbound_auth_waiting(&mut self, peer_id: PeerId, connection_id: ConnectionId) {
        if let Some(conn) = self.connections.get_mut(&connection_id) {
            // Set state to waiting for inbound auth
            conn.start_inbound_auth();

            println!(
                "Waiting for inbound auth from peer {:?} on connection {:?}",
                peer_id, connection_id
            );
        }
    }

    // Submit authentication result for a PoR verification request
    pub fn submit_por_verification_result(
        &mut self,
        connection_id: ConnectionId,
        result: AuthResult,
    ) -> Result<(), String> {
        // Get the pending verification and remove it
        let verification = match self.pending_verifications.remove(&connection_id) {
            Some(v) => v,
            None => {
                return Err(format!(
                    "No pending verification for connection ID {:?}",
                    connection_id
                ))
            }
        };

        // Send the response
        if let Err(e) = self.request_response.send_response(
            verification.response_channel,
            PorAuthResponse {
                result: result.clone(),
            },
        ) {
            return Err(format!("Failed to send auth response: {:?}", e));
        }

        // Update connection state
        if let Some(conn) = self.connections.get_mut(&connection_id) {
            conn.touch();

            // Store information we need for events
            let peer_id = conn.peer_id;
            let address = conn.address.clone();
            let need_outbound_auth;
            let is_fully_authenticated;
            let metadata_opt;

            match result {
                AuthResult::Ok(_) => {
                    // Update connection state
                    conn.set_inbound_auth_success();

                    // Check state before dropping the borrow
                    need_outbound_auth = conn.is_outbound_not_started();
                    is_fully_authenticated = matches!(
                        conn.get_combined_state(),
                        CombinedAuthState::FullyAuthenticated(_)
                    );
                    metadata_opt = conn.get_metadata();

                    // Generate inbound auth success event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::InboundAuthSuccess {
                            peer_id,
                            connection_id,
                            address: address.clone(),
                        },
                    ));

                    // Check if we now have full mutual authentication
                    if is_fully_authenticated {
                        // Generate mutual auth success event
                        if let Some(metadata) = metadata_opt {
                            self.pending_events.push_back(ToSwarm::GenerateEvent(
                                PorAuthEvent::MutualAuthSuccess {
                                    peer_id,
                                    connection_id,
                                    address: address.clone(),
                                    metadata,
                                },
                            ));
                        }
                    }
                }
                AuthResult::Error(reason) => {
                    conn.set_inbound_auth_failed(reason.clone());
                    need_outbound_auth = false;

                    // Generate outbound auth failure event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::OutboundAuthFailure {
                            peer_id,
                            connection_id,
                            address: address.clone(),
                            reason,
                        },
                    ));
                }
            }

            // If we need to start outbound auth and we're not going to get a borrow error
            if need_outbound_auth {
                // Release existing borrow
                drop(conn);
                // Now start outbound auth (which will take another mutable borrow)
                self.start_outbound_auth(peer_id, connection_id);
            }

            Ok(())
        } else {
            Err(format!(
                "Connection data not found for connection ID {:?}",
                connection_id
            ))
        }
    }

    // Start authentication in both directions for a connection
    pub fn start_authentication(&mut self, connection_id: ConnectionId) -> Result<(), String> {
        // Get the peer ID from connection data
        let peer_id = match self.connections.get(&connection_id) {
            Some(conn) => conn.peer_id,
            None => {
                return Err(format!(
                    "No connection data for connection ID {:?}",
                    connection_id
                ))
            }
        };

        // Start outbound authentication
        self.start_outbound_auth(peer_id, connection_id);

        // Begin waiting for inbound authentication
        self.start_inbound_auth_waiting(peer_id, connection_id);

        Ok(())
    }

    // Check for authentication timeouts
    fn check_timeouts(&mut self) {
        // Find connections with timeouts
        let timed_out_connections: Vec<(ConnectionId, PeerId, AuthDirection, Multiaddr)> = self
            .connections
            .iter()
            .filter_map(|(conn_id, conn)| {
                conn.check_timeout(AUTH_TIMEOUT)
                    .map(|direction| (*conn_id, conn.peer_id, direction, conn.address.clone()))
            })
            .collect();

        // Process timeouts
        for (connection_id, peer_id, direction, address) in timed_out_connections {
            if let Some(conn) = self.connections.get_mut(&connection_id) {
                // Update connection state based on timeout direction
                match direction {
                    AuthDirection::Outbound => {
                        if matches!(conn.outbound_auth, DirectionalAuthState::InProgress { .. }) {
                            conn.set_outbound_auth_failed(
                                "Outbound authentication timed out".to_string(),
                            );
                        }
                    }
                    AuthDirection::Inbound => {
                        if matches!(conn.inbound_auth, DirectionalAuthState::InProgress { .. }) {
                            conn.set_inbound_auth_failed(
                                "Inbound authentication timed out".to_string(),
                            );
                        }
                    }
                    AuthDirection::Both => {
                        // Set both as failed if they were in progress
                        if matches!(conn.outbound_auth, DirectionalAuthState::InProgress { .. }) {
                            conn.set_outbound_auth_failed("Authentication timed out".to_string());
                        }

                        if matches!(conn.inbound_auth, DirectionalAuthState::InProgress { .. }) {
                            conn.set_inbound_auth_failed("Authentication timed out".to_string());
                        }
                    }
                }

                // Generate timeout event
                self.pending_events
                    .push_back(ToSwarm::GenerateEvent(PorAuthEvent::AuthTimeout {
                        peer_id,
                        connection_id,
                        address,
                        direction: direction.clone(),
                    }));

                println!(
                    "Authentication timeout for peer {:?} on connection {:?}: {:?}",
                    peer_id, connection_id, direction
                );
            }
        }

        // Also check for verification timeouts
        let timeout = Duration::from_secs(30); // 30 seconds timeout for verifications
        let now = Instant::now();

        let timed_out_verifications: Vec<ConnectionId> = self
            .pending_verifications
            .iter()
            .filter_map(|(conn_id, verification)| {
                if now.duration_since(verification.received_at) > timeout {
                    Some(*conn_id)
                } else {
                    None
                }
            })
            .collect();

        // Remove timed out verifications
        for conn_id in timed_out_verifications {
            if let Some(verification) = self.pending_verifications.remove(&conn_id) {
                println!(
                    "PoR verification for peer {:?} on connection {:?} timed out",
                    verification.peer_id, conn_id
                );

                // Try to send a timeout response
                let _ = self.request_response.send_response(
                    verification.response_channel,
                    PorAuthResponse {
                        result: AuthResult::Error("Verification timed out".to_string()),
                    },
                );
            }
        }
    }

    // Get current authentication state for a peer
    pub fn is_peer_authenticated(&self, peer_id: &PeerId) -> bool {
        // Check if any connection for this peer is fully authenticated
        if let Some(connection_ids) = self.peer_connections.get(peer_id) {
            for conn_id in connection_ids {
                if let Some(conn) = self.connections.get(conn_id) {
                    if conn.is_fully_authenticated() {
                        return true;
                    }
                }
            }
        }
        false
    }

    // Get peer's authentication metadata if available
    pub fn get_peer_metadata(&self, peer_id: &PeerId) -> Option<HashMap<String, String>> {
        // Try to find metadata from any authenticated connection for this peer
        if let Some(connection_ids) = self.peer_connections.get(peer_id) {
            for conn_id in connection_ids {
                if let Some(conn) = self.connections.get(conn_id) {
                    if let Some(metadata) = conn.get_metadata() {
                        return Some(metadata);
                    }
                }
            }
        }
        None
    }

    // Get a pending verification by peer_id
    pub fn get_pending_verification(
        &self,
        peer_id: &PeerId,
    ) -> Option<(ConnectionId, &PendingVerification)> {
        for (conn_id, verification) in &self.pending_verifications {
            if &verification.peer_id == peer_id {
                return Some((*conn_id, verification));
            }
        }
        None
    }
}

// Implement NetworkBehaviour manually
impl NetworkBehaviour for PorAuthBehaviour {
    type ConnectionHandler = <request_response::cbor::Behaviour<PorAuthRequest, PorAuthResponse> as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = PorAuthEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Pass through to the underlying request_response behaviour
        match self.request_response.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        ) {
            Ok(handler) => {
                println!(
                    "Established inbound connection with peer: {:?} (connection ID: {:?})",
                    peer, connection_id
                );

                // Store the connection for our tracking
                let conn_data = ConnectionData::new(peer, connection_id, remote_addr.clone());
                self.connections.insert(connection_id, conn_data);

                // Update peer to connection mapping
                self.peer_connections
                    .entry(peer)
                    .or_insert_with(HashSet::new)
                    .insert(connection_id);

                Ok(handler)
            }
            Err(e) => Err(e),
        }
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: libp2p::core::transport::PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Pass through to the underlying request_response behaviour
        match self
            .request_response
            .handle_established_outbound_connection(
                connection_id,
                peer,
                addr,
                role_override,
                port_use,
            ) {
            Ok(handler) => {
                println!(
                    "Established outbound connection with peer: {:?} (connection ID: {:?})",
                    peer, connection_id
                );

                // Store the connection for our tracking
                let conn_data = ConnectionData::new(peer, connection_id, addr.clone());
                self.connections.insert(connection_id, conn_data);

                // Update peer to connection mapping
                self.peer_connections
                    .entry(peer)
                    .or_insert_with(HashSet::new)
                    .insert(connection_id);

                Ok(handler)
            }
            Err(e) => Err(e),
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        // Process the underlying request_response event
        self.request_response
            .on_connection_handler_event(peer_id, connection_id, event);
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Pass through to the underlying request_response behaviour
        self.request_response.on_swarm_event(event.clone());

        match event {
            FromSwarm::ConnectionEstablished(connection_established) => {
                println!(
                    "Connection established with peer: {:?} (connection ID: {:?})",
                    connection_established.peer_id, connection_established.connection_id
                );

                // Begin authentication process immediately on connection establishment
                // We don't need to check if the connection exists, we can just try to start auth
                let _ = self.start_authentication(connection_established.connection_id);
            }
            FromSwarm::ConnectionClosed(connection_closed) => {
                // Clean up when a connection is closed
                if let Some(conn) = self.connections.remove(&connection_closed.connection_id) {
                    // Also update the peer_connections map
                    if let Some(connections) = self.peer_connections.get_mut(&conn.peer_id) {
                        connections.remove(&connection_closed.connection_id);
                        if connections.is_empty() {
                            self.peer_connections.remove(&conn.peer_id);
                        }
                    }

                    // Remove any pending verification for this connection
                    self.pending_verifications
                        .remove(&connection_closed.connection_id);
                }
            }
            _ => {}
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Check for timeouts
        self.check_timeouts();

        // First, poll the request_response behaviour
        if let Poll::Ready(event) = self.request_response.poll(cx) {
            match event {
                ToSwarm::GenerateEvent(request_response::Event::Message {
                    message:
                        request_response::Message::Request {
                            request, channel, ..
                        },
                    peer,
                    connection_id, // Fixed field name
                    ..
                }) => {
                    // Handle incoming auth request with connection ID
                    self.handle_auth_request(peer, connection_id, request, channel);
                    return Poll::Pending;
                }
                ToSwarm::GenerateEvent(request_response::Event::Message {
                    message: request_response::Message::Response { response, .. },
                    peer,
                    connection_id, // Fixed field name
                    ..
                }) => {
                    // Handle auth response with connection ID
                    self.handle_auth_response(peer, connection_id, response);
                    return Poll::Pending;
                }
                ToSwarm::GenerateEvent(request_response::Event::OutboundFailure {
                    peer,
                    connection_id, // Fixed field name
                    error,
                    ..
                }) => {
                    println!(
                        "Outbound failure for peer {:?} on connection {:?}: {:?}",
                        peer, connection_id, error
                    );

                    // Mark connection as failed if it exists
                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&connection_id) {
                        conn.set_outbound_auth_failed(format!(
                            "Outbound request failed: {:?}",
                            error
                        ));

                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(
                            PorAuthEvent::InboundAuthFailure {
                                peer_id: peer,
                                connection_id,
                                address,
                                reason: format!("Outbound request failed: {:?}", error),
                            },
                        ));
                    }
                    return Poll::Pending;
                }
                ToSwarm::GenerateEvent(request_response::Event::InboundFailure {
                    peer,
                    connection_id, // Fixed field name
                    error,
                    ..
                }) => {
                    println!(
                        "Inbound failure for peer {:?} on connection {:?}: {:?}",
                        peer, connection_id, error
                    );

                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&connection_id) {
                        conn.set_inbound_auth_failed(format!(
                            "Inbound request failed: {:?}",
                            error
                        ));

                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(
                            PorAuthEvent::OutboundAuthFailure {
                                peer_id: peer,
                                connection_id,
                                address,
                                reason: format!("Inbound request failed: {:?}", error),
                            },
                        ));
                    }
                    return Poll::Pending;
                }
                ToSwarm::NotifyHandler {
                    peer_id,
                    handler,
                    event,
                } => {
                    // Pass events to handler
                    return Poll::Ready(ToSwarm::NotifyHandler {
                        peer_id,
                        handler,
                        event,
                    });
                }
                _ => {
                    // We'll ignore other events and continue processing
                    return Poll::Pending;
                }
            }
        }

        // Process any pending events
        if let Some(event) = self.pending_events.pop_front() {
            match event {
                ToSwarm::GenerateEvent(ev) => {
                    return Poll::Ready(ToSwarm::GenerateEvent(ev));
                }
                _ => {
                    // This shouldn't happen as we only push GenerateEvent to the queue
                    return Poll::Pending;
                }
            }
        }

        Poll::Pending
    }
}

use libp2p::{
    core::{Endpoint, Multiaddr},
    request_response::{self, ResponseChannel},
    swarm::{
        derive_prelude::*, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, ToSwarm,
    },
    PeerId, StreamProtocol,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    task::{Context, Poll},
    time::{Duration, Instant},
};

// Import the ProofOfRepresentation from the por module
use super::por::por::ProofOfRepresentation;

// Protocol identifier
pub const PROTOCOL_ID: &str = "/por-auth/1.0.0";
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(5);

// Authentication messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PorAuthRequest {
    pub por: ProofOfRepresentation,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PorAuthResponse {
    pub result: AuthResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthResult {
    Ok(HashMap<String, String>),
    Error(String),
}

// Events emitted by the behaviour
#[derive(Debug)]
pub enum PorAuthEvent {
    // Authentication succeeded in both directions
    MutualAuthSuccess {
        peer_id: PeerId,
        address: Multiaddr,
        metadata: HashMap<String, String>,
    },
    // We authenticated the remote peer
    OutboundAuthSuccess {
        peer_id: PeerId,
        address: Multiaddr,
        metadata: HashMap<String, String>,
    },
    // Remote peer authenticated us
    InboundAuthSuccess {
        peer_id: PeerId,
        address: Multiaddr,
    },
    // We rejected the remote peer's authentication
    OutboundAuthFailure {
        peer_id: PeerId,
        address: Multiaddr,
        reason: String,
    },
    // Remote peer rejected our authentication
    InboundAuthFailure {
        peer_id: PeerId,
        address: Multiaddr,
        reason: String,
    },
    // Authentication timeout
    AuthTimeout {
        peer_id: PeerId,
        address: Multiaddr,
        direction: AuthDirection,
    },
    // PoR verification needed
    VerifyPorRequest {
        peer_id: PeerId,
        address: Multiaddr,
        por: ProofOfRepresentation,
        metadata: HashMap<String, String>,
        response_channel: ResponseChannel<PorAuthResponse>,
    },
}

#[derive(Debug, Clone)]
pub enum AuthDirection {
    Inbound,  // From remote peer to us
    Outbound, // From us to remote peer
    Both,     // In both directions
}

// New enum types for individual authentication directions
#[derive(Debug, Clone, PartialEq)]
enum DirectionalAuthState {
    // Not yet started auth process in this direction
    NotStarted,
    // Authentication in progress
    InProgress { started: Instant },
    // Authentication succeeded
    Successful(HashMap<String, String>),
    // Authentication failed
    Failed(String),
}

// Compute combined authentication state
#[derive(Debug, Clone, PartialEq)]
enum CombinedAuthState {
    // Not authenticated in either direction
    NotAuthenticated,
    // Only outbound authenticated
    OutboundOnly(HashMap<String, String>),
    // Only inbound authenticated
    InboundOnly,
    // Full mutual authentication
    FullyAuthenticated(HashMap<String, String>),
    // Authentication has failed
    Failed(String),
}

// Improved connection data structure
struct ConnectionData {
    // Connection metadata
    address: Multiaddr,
    // When the connection was established
    established: Instant,
    // Last activity timestamp
    last_activity: Instant,
    // Inbound authentication state (remote peer authenticating us)
    inbound_auth: DirectionalAuthState,
    // Outbound authentication state (us authenticating remote peer)
    outbound_auth: DirectionalAuthState,
}

impl ConnectionData {
    // Create a new connection data
    fn new(address: Multiaddr) -> Self {
        let now = Instant::now();
        Self {
            address,
            established: now,
            last_activity: now,
            inbound_auth: DirectionalAuthState::NotStarted,
            outbound_auth: DirectionalAuthState::NotStarted,
        }
    }

    // Update activity timestamp
    fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    // Start outbound authentication
    fn start_outbound_auth(&mut self) {
        self.outbound_auth = DirectionalAuthState::InProgress {
            started: Instant::now(),
        };
        self.touch();
    }

    // Start inbound authentication
    fn start_inbound_auth(&mut self) {
        self.inbound_auth = DirectionalAuthState::InProgress {
            started: Instant::now(),
        };
        self.touch();
    }

    // Set outbound auth as successful
    fn set_outbound_auth_success(&mut self, metadata: HashMap<String, String>) {
        self.outbound_auth = DirectionalAuthState::Successful(metadata);
        self.touch();
    }

    // Set inbound auth as successful
    fn set_inbound_auth_success(&mut self) {
        self.inbound_auth = DirectionalAuthState::Successful(HashMap::new());
        self.touch();
    }

    // Set outbound auth as failed
    fn set_outbound_auth_failed(&mut self, reason: String) {
        self.outbound_auth = DirectionalAuthState::Failed(reason);
        self.touch();
    }

    // Set inbound auth as failed
    fn set_inbound_auth_failed(&mut self, reason: String) {
        self.inbound_auth = DirectionalAuthState::Failed(reason);
        self.touch();
    }

    // Get the current combined auth state
    fn get_combined_state(&self) -> CombinedAuthState {
        match (&self.inbound_auth, &self.outbound_auth) {
            // If either direction has failed, the combined state is Failed
            (DirectionalAuthState::Failed(reason), _) => CombinedAuthState::Failed(reason.clone()),
            (_, DirectionalAuthState::Failed(reason)) => CombinedAuthState::Failed(reason.clone()),

            // If both directions are successful, the connection is fully authenticated
            (DirectionalAuthState::Successful(_), DirectionalAuthState::Successful(metadata)) => {
                CombinedAuthState::FullyAuthenticated(metadata.clone())
            }

            // If only one direction is authenticated
            (DirectionalAuthState::Successful(_), _) => CombinedAuthState::InboundOnly,
            (_, DirectionalAuthState::Successful(metadata)) => {
                CombinedAuthState::OutboundOnly(metadata.clone())
            }

            // Otherwise, not authenticated
            _ => CombinedAuthState::NotAuthenticated,
        }
    }

    // Check for authentication timeouts
    fn check_timeout(&self, auth_timeout: Duration) -> Option<AuthDirection> {
        let now = Instant::now();

        // Check each authentication direction for timeout
        let inbound_timeout = match &self.inbound_auth {
            DirectionalAuthState::InProgress { started }
                if now.duration_since(*started) > auth_timeout =>
            {
                true
            }
            _ => false,
        };

        let outbound_timeout = match &self.outbound_auth {
            DirectionalAuthState::InProgress { started }
                if now.duration_since(*started) > auth_timeout =>
            {
                true
            }
            _ => false,
        };

        // Check for overall connection inactivity
        let conn_timeout = now.duration_since(self.last_activity) > auth_timeout * 1;

        // Return appropriate timeout direction

        if conn_timeout && !self.is_fully_authenticated() {
            Some(AuthDirection::Both)
        } else if inbound_timeout && outbound_timeout {
            Some(AuthDirection::Both)
        } else if inbound_timeout {
            Some(AuthDirection::Inbound)
        } else if outbound_timeout {
            Some(AuthDirection::Outbound)
        } else {
            None
        }
    }

    // Get peer's authentication metadata if available
    fn get_metadata(&self) -> Option<HashMap<String, String>> {
        match &self.outbound_auth {
            DirectionalAuthState::Successful(metadata) => Some(metadata.clone()),
            _ => None,
        }
    }

    // Check if peer is fully authenticated
    fn is_fully_authenticated(&self) -> bool {
        matches!(
            self.get_combined_state(),
            CombinedAuthState::FullyAuthenticated(_)
        )
    }

    // Check if outbound auth is not started
    fn is_outbound_not_started(&self) -> bool {
        matches!(self.outbound_auth, DirectionalAuthState::NotStarted)
    }
}

// Define the behaviour
pub struct PorAuthBehaviour {
    // Using cbor codec for request-response
    pub request_response: request_response::cbor::Behaviour<PorAuthRequest, PorAuthResponse>,

    // Improved connection tracking
    connections: HashMap<PeerId, ConnectionData>,

    // Events to be emitted
    pending_events: VecDeque<ToSwarm<PorAuthEvent, request_response::OutboundRequestId>>,

    // Authentication data (PoR)
    por: ProofOfRepresentation,

    // Additional metadata to send with auth request
    metadata: HashMap<String, String>,
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
            pending_events: VecDeque::new(),
            por,
            metadata,
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

    // Handle incoming authentication request by delegating to the application
    fn handle_auth_request(
        &mut self,
        peer_id: PeerId,
        request: PorAuthRequest,
        channel: ResponseChannel<PorAuthResponse>,
    ) {
        // Log the received authentication request
        println!("Received auth request from {:?}", peer_id);

        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.touch();

            // Forward the PoR verification request to the application
            let address = conn.address.clone();
            self.pending_events
                .push_back(ToSwarm::GenerateEvent(PorAuthEvent::VerifyPorRequest {
                    peer_id,
                    address,
                    por: request.por,
                    metadata: request.metadata,
                    response_channel: channel,
                }));
        }
    }

    // Process the authentication response from the remote peer
    fn handle_auth_response(&mut self, peer_id: PeerId, response: PorAuthResponse) {
        println!(
            "Received auth response from {:?}: {:?}",
            peer_id, response.result
        );

        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.touch();

            match response.result {
                AuthResult::Ok(metadata) => {
                    // Update connection state
                    conn.set_outbound_auth_success(metadata.clone());

                    // Generate outbound auth success event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::OutboundAuthSuccess {
                            peer_id,
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
                            address: conn.address.clone(),
                            reason,
                        },
                    ));
                }
            }
        }
    }

    // Start outbound authentication process with a peer
    fn start_outbound_auth(&mut self, peer_id: PeerId) {
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            // Update state
            conn.start_outbound_auth();

            println!("Starting outbound auth with peer {:?}", peer_id);

            // Send authentication request
            let request = PorAuthRequest {
                por: self.por.clone(),
                metadata: self.metadata.clone(),
            };

            self.request_response.send_request(&peer_id, request);
        }
    }

    // Start inbound authentication waiting
    fn start_inbound_auth_waiting(&mut self, peer_id: PeerId) {
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            // Set state to waiting for inbound auth
            conn.start_inbound_auth();

            println!("Waiting for inbound auth from peer {:?}", peer_id);
        }
    }

    // Submit authentication result for a PoR verification request
    pub fn submit_por_verification_result(
        &mut self,
        peer_id: PeerId,
        result: AuthResult,
        channel: ResponseChannel<PorAuthResponse>,
    ) {
        // Send the response immediately
        if let Err(e) = self.request_response.send_response(
            channel,
            PorAuthResponse {
                result: result.clone(),
            },
        ) {
            println!("Failed to send auth response: {:?}", e);
            return;
        }

        // Update connection state
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.touch();

            match result {
                AuthResult::Ok(_) => {
                    // Update connection state
                    conn.set_inbound_auth_success();

                    // Generate inbound auth success event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::InboundAuthSuccess {
                            peer_id,
                            address: conn.address.clone(),
                        },
                    ));

                    // Check if we need to start outbound auth
                    let need_outbound_auth = conn.is_outbound_not_started();

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
                                    address: conn.address.clone(),
                                    metadata,
                                },
                            ));
                        }
                    }

                    // If we have not started outbound auth yet, do it now
                    if need_outbound_auth {
                        self.start_outbound_auth(peer_id);
                    }
                }
                AuthResult::Error(reason) => {
                    conn.set_inbound_auth_failed(reason.clone());

                    // Generate outbound auth failure event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        PorAuthEvent::OutboundAuthFailure {
                            peer_id,
                            address: conn.address.clone(),
                            reason: reason.clone(),
                        },
                    ));
                }
            }
        }
    }

    // Start authentication in both directions
    pub fn start_authentication(&mut self, peer_id: &PeerId) {
        // Start outbound authentication
        self.start_outbound_auth(*peer_id);

        // Begin waiting for inbound authentication
        self.start_inbound_auth_waiting(*peer_id);
    }

    // Check for authentication timeouts
    fn check_timeouts(&mut self) {
        // Find connections with timeouts
        let timed_out_peers: Vec<(PeerId, AuthDirection)> = self
            .connections
            .iter()
            .filter_map(|(peer_id, conn)| {
                conn.check_timeout(AUTH_TIMEOUT)
                    .map(|direction| (*peer_id, direction))
            })
            .collect();

        // Process timeouts
        for (peer_id, direction) in timed_out_peers {
            if let Some(conn) = self.connections.get_mut(&peer_id) {
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
                        address: conn.address.clone(),
                        direction: direction.clone(),
                    }));

                println!(
                    "Authentication timeout for peer {:?}: {:?}",
                    peer_id, direction
                );
            }
        }
    }

    // Get current authentication state for a peer
    pub fn is_peer_authenticated(&self, peer_id: &PeerId) -> bool {
        if let Some(conn) = self.connections.get(peer_id) {
            conn.is_fully_authenticated()
        } else {
            false
        }
    }

    // Get peer's authentication metadata if available
    pub fn get_peer_metadata(&self, peer_id: &PeerId) -> Option<HashMap<String, String>> {
        if let Some(conn) = self.connections.get(peer_id) {
            conn.get_metadata()
        } else {
            None
        }
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
                println!("Established inbound connection with peer: {:?}", peer);

                // Store the connection for our tracking
                self.connections
                    .insert(peer, ConnectionData::new(remote_addr.clone()));
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
                println!("Established outbound connection with peer: {:?}", peer);

                // Store the connection for our tracking
                self.connections
                    .insert(peer, ConnectionData::new(addr.clone()));
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
                    "Connection established with peer: {:?}",
                    connection_established.peer_id
                );

                // Store the connection if not already stored
                if !self
                    .connections
                    .contains_key(&connection_established.peer_id)
                {
                    self.connections.insert(
                        connection_established.peer_id,
                        ConnectionData::new(
                            connection_established.endpoint.get_remote_address().clone(),
                        ),
                    );
                }

                // Begin authentication process immediately on connection establishment
                self.start_authentication(&connection_established.peer_id);
            }
            FromSwarm::ConnectionClosed(connection_closed) => {
                // Clean up when a connection is closed
                if connection_closed.remaining_established == 0 {
                    self.connections.remove(&connection_closed.peer_id);
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
                    ..
                }) => {
                    // Handle incoming auth request
                    self.handle_auth_request(peer, request, channel);
                    return Poll::Pending;
                }
                ToSwarm::GenerateEvent(request_response::Event::Message {
                    message: request_response::Message::Response { response, .. },
                    peer,
                    ..
                }) => {
                    // Handle auth response
                    self.handle_auth_response(peer, response);
                    return Poll::Pending;
                }
                ToSwarm::GenerateEvent(request_response::Event::OutboundFailure {
                    peer,
                    error,
                    ..
                }) => {
                    println!("Outbound failure for peer {:?}: {:?}", peer, error);

                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.set_outbound_auth_failed(format!(
                            "Outbound request failed: {:?}",
                            error
                        ));

                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(
                            PorAuthEvent::InboundAuthFailure {
                                peer_id: peer,
                                address,
                                reason: format!("Outbound request failed: {:?}", error),
                            },
                        ));
                    }
                    return Poll::Pending;
                }
                ToSwarm::GenerateEvent(request_response::Event::InboundFailure {
                    peer,
                    error,
                    ..
                }) => {
                    println!("Inbound failure for peer {:?}: {:?}", peer, error);

                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.set_inbound_auth_failed(format!(
                            "Inbound request failed: {:?}",
                            error
                        ));

                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(
                            PorAuthEvent::OutboundAuthFailure {
                                peer_id: peer,
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

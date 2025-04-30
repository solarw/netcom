use libp2p::{
    core::{Endpoint, Multiaddr},
    request_response::{self, ResponseChannel},
    swarm::{
        NetworkBehaviour, ConnectionId, FromSwarm, ToSwarm, ConnectionDenied,
        derive_prelude::*,
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

// Protocol identifier
pub const PROTOCOL_ID: &str = "/xauth/1.0.0";
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(15);

// Authentication messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest(pub HashMap<String, String>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse(pub AuthResult);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthResult {
    Ok,
    Error(String),
}

// Events emitted by the behaviour
#[derive(Debug)]
pub enum XAuthEvent {
    InboundRequest {
        request: HashMap<String, String>,
        peer: PeerId,
        channel: ResponseChannel<AuthResponse>,
    },
    Success {
        peer_id: PeerId,
        address: Multiaddr,
    },
    Failure {
        peer_id: PeerId,
        address: Multiaddr,
        reason: String,
    },
    Timeout {
        peer_id: PeerId,
        address: Multiaddr,
    },
}

// Authentication state for a connection
#[derive(Debug, Clone, PartialEq)]
enum AuthState {
    NotAuthenticated,
    AuthenticationInProgress { started: Instant },
    Authenticated,
    Failed(String),
}

// Connection data structure to track auth state
struct ConnectionData {
    address: Multiaddr,
    state: AuthState,
}

// Define the behaviour
pub struct XAuthBehaviour {
    // Using cbor codec for request-response
    pub request_response: request_response::cbor::Behaviour<AuthRequest, AuthResponse>,
    
    // Additional state for tracking connections
    connections: HashMap<PeerId, ConnectionData>,
    
    // Events to be emitted
    pending_events: VecDeque<ToSwarm<XAuthEvent, request_response::OutboundRequestId>>,
}

impl XAuthBehaviour {
    pub fn new() -> Self {
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
        }
    }

    // Handle incoming authentication request
    pub fn handle_auth_request(&mut self, peer_id: PeerId, data: HashMap<String, String>) -> AuthResult {
        // Simple authentication check
        let result = if data.get("hello") == Some(&"world".to_string()) {
            AuthResult::Ok
        } else {
            AuthResult::Error("Invalid authentication data".to_string())
        };

        if let Some(conn) = self.connections.get_mut(&peer_id) {
            // Update connection state based on auth result
            match &result {
                AuthResult::Ok => {
                    conn.state = AuthState::Authenticated;
                    self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Success {
                        peer_id,
                        address: conn.address.clone(),
                    }));
                }
                AuthResult::Error(reason) => {
                    conn.state = AuthState::Failed(reason.clone());
                    self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Failure {
                        peer_id,
                        address: conn.address.clone(),
                        reason: reason.clone(),
                    }));
                }
            }
        }

        result
    }

    // Handle authentication response
    pub fn handle_auth_response(&mut self, peer_id: PeerId, result: AuthResult) {
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            match result {
                AuthResult::Ok => {
                    conn.state = AuthState::Authenticated;
                    self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Success {
                        peer_id,
                        address: conn.address.clone(),
                    }));
                }
                AuthResult::Error(reason) => {
                    conn.state = AuthState::Failed(reason.clone());
                    self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Failure {
                        peer_id,
                        address: conn.address.clone(),
                        reason,
                    }));
                }
            }
        }
    }

    // Start authentication process with a peer
    pub fn authenticate_peer(&mut self, peer_id: &PeerId) {
        if let Some(conn) = self.connections.get_mut(peer_id) {
            conn.state = AuthState::AuthenticationInProgress { started: Instant::now() };
            
            // Create default auth data
            let mut auth_data = HashMap::new();
            auth_data.insert("hello".to_string(), "world".to_string());
            println!("111111111111 Sending auth request to {:?}: {:?}", peer_id, auth_data);
            // Send request using request_response protocol
            self.request_response.send_request(peer_id, AuthRequest(auth_data));
        }
    }

    // Check for authentication timeouts
    fn check_timeouts(&mut self) {
        let now = Instant::now();
        let timed_out_peers: Vec<PeerId> = self.connections.iter()
            .filter_map(|(peer_id, conn)| {
                if let AuthState::AuthenticationInProgress { started } = conn.state {
                    if now.duration_since(started) > AUTH_TIMEOUT {
                        Some(*peer_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for peer_id in timed_out_peers {
            if let Some(conn) = self.connections.get_mut(&peer_id) {
                conn.state = AuthState::Failed("Authentication timed out".to_string());
                let address = conn.address.clone();
                self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Timeout {
                    peer_id,
                    address,
                }));
            }
        }
    }
}

// Implement NetworkBehaviour manually
impl NetworkBehaviour for XAuthBehaviour {
    type ConnectionHandler = <request_response::cbor::Behaviour<AuthRequest, AuthResponse> as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = XAuthEvent;

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
                // Store the connection for our tracking
                self.connections.insert(peer, ConnectionData {
                    address: remote_addr.clone(),
                    state: AuthState::NotAuthenticated,
                });
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
        port_reuse: libp2p::core::transport::PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Pass through to the underlying request_response behaviour
        match self.request_response.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_reuse,
        ) {
            Ok(handler) => {
                // Store the connection for our tracking
                self.connections.insert(peer, ConnectionData {
                    address: addr.clone(),
                    state: AuthState::NotAuthenticated,
                });
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
        self.request_response.on_connection_handler_event(peer_id, connection_id, event);

        // Check if the current peer needs authentication
        if let Some(connection) = self.connections.get(&peer_id) {
            if let AuthState::NotAuthenticated = connection.state {
                // Initiate authentication for this peer
                self.authenticate_peer(&peer_id);
            }
        }
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Pass through to the underlying request_response behaviour
        self.request_response.on_swarm_event(event.clone());

        match event {
            FromSwarm::ConnectionEstablished(connection_established) => {
                // Store the connection
                self.connections.insert(
                    connection_established.peer_id,
                    ConnectionData {
                        address: connection_established.endpoint.get_remote_address().clone(),
                        state: AuthState::NotAuthenticated,
                    },
                );
                
                // Begin authentication process on connection establishment
                self.authenticate_peer(&connection_established.peer_id);
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
                    message: request_response::Message::Request { request, channel, .. },
                    peer,
                    ..
                }) => {
                    // Handle incoming request
                    return Poll::Ready(ToSwarm::GenerateEvent(XAuthEvent::InboundRequest {
                        request: request.0,
                        peer,
                        channel,
                    }));
                }
                ToSwarm::GenerateEvent(request_response::Event::Message { 
                    message: request_response::Message::Response { response, .. },
                    peer,
                    ..
                }) => {
                    // Handle response
                    self.handle_auth_response(peer, response.0);
                    // Continue polling to get our generated events
                }
                ToSwarm::GenerateEvent(request_response::Event::OutboundFailure { peer, error, .. }) => {
                    // Здесь логируйте точную ошибку
                    println!("Outbound failure for peer {:?}: {:?}", peer, error);
                    
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.state = AuthState::Failed(format!("Outbound request failed: {:?}", error));
                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Failure {
                            peer_id: peer,
                            address,
                            reason: format!("Outbound request failed: {:?}", error),
                        }));
                    }
                }

                ToSwarm::GenerateEvent(request_response::Event::InboundFailure { peer, .. }) => {
                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.state = AuthState::Failed("Inbound request failed".to_string());
                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::Failure {
                            peer_id: peer,
                            address,
                            reason: "Inbound request failed".to_string(),
                        }));
                    }
                }
                ToSwarm::NotifyHandler { peer_id, handler, event } => {
                    return Poll::Ready(ToSwarm::NotifyHandler {
                        peer_id,
                        handler,
                        event,
                    });
                }
                _ => {
                    // Other events are passed through
                }
            }
        }
        
        // Then, process any events we have
        if let Some(event) = self.pending_events.pop_front() {
            // Convert event to the correct type if needed
            match event {
                ToSwarm::GenerateEvent(evt) => return Poll::Ready(ToSwarm::GenerateEvent(evt)),
                _ => {}
            }
        }
        
        Poll::Pending
    }
}
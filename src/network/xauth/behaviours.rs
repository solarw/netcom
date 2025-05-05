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
    Inbound,    // From remote peer to us
    Outbound,   // From us to remote peer
    Both,       // In both directions
}

// Authentication state for a connection
#[derive(Debug, Clone, PartialEq)]
enum AuthState {
    NotAuthenticated,
    // Waiting for response to our auth request
    OutboundAuthInProgress { started: Instant },
    // Waiting for inbound auth request
    InboundAuthInProgress { started: Instant },
    // We authenticated the remote peer, but they haven't authenticated us
    OutboundAuthSuccessful(HashMap<String, String>),
    // Remote peer authenticated us, but we haven't authenticated them
    InboundAuthSuccessful,
    // Full mutual authentication
    FullyAuthenticated(HashMap<String, String>),
    // Authentication failed
    Failed(String),
}

// Connection data structure to track auth state
struct ConnectionData {
    address: Multiaddr,
    state: AuthState,
    // Time of last activity on this connection
    last_activity: Instant,
}

// Define the behaviour
pub struct PorAuthBehaviour {
    // Using cbor codec for request-response
    pub request_response: request_response::cbor::Behaviour<PorAuthRequest, PorAuthResponse>,
    
    // Additional state for tracking connections
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
        channel: ResponseChannel<PorAuthResponse>
    ) {
        // Log the received authentication request
        println!("Received auth request from {:?}", peer_id);
        
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.last_activity = Instant::now();
            
            // Forward the PoR verification request to the application
            if let Some(address) = self.connections.get(&peer_id).map(|c| c.address.clone()) {
                self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::VerifyPorRequest {
                    peer_id,
                    address,
                    por: request.por,
                    metadata: request.metadata,
                    response_channel: channel,
                }));
            }
        }
    }

    // Process the authentication response from the remote peer
    fn handle_auth_response(&mut self, peer_id: PeerId, response: PorAuthResponse) {
        println!("Received auth response from {:?}: {:?}", peer_id, response.result);
        
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.last_activity = Instant::now();
            
            match response.result {
                AuthResult::Ok(metadata) => {
                    // Update state based on current status
                    match &conn.state {
                        AuthState::OutboundAuthInProgress { .. } => {
                            conn.state = AuthState::OutboundAuthSuccessful(metadata.clone());
                            
                            // Generate outbound auth success event
                            self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::OutboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                                metadata: metadata.clone(),
                            }));
                            
                            // If we already have inbound auth, update to fully authenticated
                            if let AuthState::InboundAuthSuccessful = conn.state {
                                conn.state = AuthState::FullyAuthenticated(metadata.clone());
                                
                                // Generate mutual auth success event
                                self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::MutualAuthSuccess {
                                    peer_id,
                                    address: conn.address.clone(),
                                    metadata,
                                }));
                            }
                        }
                        AuthState::InboundAuthSuccessful => {
                            // Now we have full mutual authentication
                            conn.state = AuthState::FullyAuthenticated(metadata.clone());
                            
                            // Generate mutual auth success event
                            self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::MutualAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                                metadata,
                            }));
                        }
                        _ => {
                            // Other states shouldn't occur here, but just in case
                            conn.state = AuthState::OutboundAuthSuccessful(metadata.clone());
                            
                            self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::OutboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                                metadata,
                            }));
                        }
                    }
                }
                AuthResult::Error(reason) => {
                    conn.state = AuthState::Failed(reason.clone());
                    
                    // Generate inbound auth failure event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::InboundAuthFailure {
                        peer_id,
                        address: conn.address.clone(),
                        reason,
                    }));
                }
            }
        }
    }

    // Start outbound authentication process with a peer
    fn start_outbound_auth(&mut self, peer_id: PeerId) {
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            // Update state
            conn.state = AuthState::OutboundAuthInProgress { started: Instant::now() };
            conn.last_activity = Instant::now();
            
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
            conn.state = AuthState::InboundAuthInProgress { started: Instant::now() };
            conn.last_activity = Instant::now();
            
            println!("Waiting for inbound auth from peer {:?}", peer_id);
        }
    }

    // Submit authentication result for a PoR verification request
    pub fn submit_por_verification_result(
        &mut self, 
        peer_id: PeerId, 
        result: AuthResult, 
        channel: ResponseChannel<PorAuthResponse>
    ) {
        // Send the response immediately
        if let Err(e) = self.request_response.send_response(channel, PorAuthResponse { result: result.clone() }) {
            println!("Failed to send auth response: {:?}", e);
            return;
        }

        // Update connection state
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.last_activity = Instant::now();
            
            match result {
                AuthResult::Ok(metadata) => {
                    // Update state based on current status
                    match conn.state {
                        AuthState::NotAuthenticated | AuthState::InboundAuthInProgress { .. } => {
                            conn.state = AuthState::InboundAuthSuccessful;
                            
                            // Generate inbound auth success event
                            self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::InboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                            
                            // If we haven't sent an auth request yet, do it now
                            if let AuthState::InboundAuthSuccessful = conn.state {
                                self.start_outbound_auth(peer_id);
                            }
                        }
                        AuthState::OutboundAuthSuccessful(ref meta) => {
                            // Generate mutual auth success event
                            self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::MutualAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                                metadata: meta.clone(),
                            }));
                            
                            // Now we have full mutual authentication
                            conn.state = AuthState::FullyAuthenticated(meta.clone());
                        }
                        _ => {
                            // Other states shouldn't occur here, but just in case
                            conn.state = AuthState::InboundAuthSuccessful;
                            
                            self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::InboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                        }
                    }
                }
                AuthResult::Error(reason) => {
                    conn.state = AuthState::Failed(reason.clone());
                    
                    // Generate outbound auth failure event
                    self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::OutboundAuthFailure {
                        peer_id,
                        address: conn.address.clone(),
                        reason: reason.clone(),
                    }));
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
        let now = Instant::now();
        
        // Find connections with timeouts
        let timed_out_peers: Vec<(PeerId, AuthDirection)> = self.connections.iter()
            .filter_map(|(peer_id, conn)| {
                match conn.state {
                    // Outbound auth with timeout
                    AuthState::OutboundAuthInProgress { started } 
                        if now.duration_since(started) > AUTH_TIMEOUT => {
                        Some((*peer_id, AuthDirection::Outbound))
                    },
                    // Inbound auth with timeout
                    AuthState::InboundAuthInProgress { started }
                        if now.duration_since(started) > AUTH_TIMEOUT => {
                        Some((*peer_id, AuthDirection::Inbound))
                    },
                    // Inactive connection timeout
                    _ if now.duration_since(conn.last_activity) > AUTH_TIMEOUT * 2 => {
                        Some((*peer_id, AuthDirection::Both))
                    },
                    _ => None,
                }
            })
            .collect();

        // Process timeouts
        for (peer_id, direction) in timed_out_peers {
            if let Some(conn) = self.connections.get_mut(&peer_id) {
                // Update connection state
                match direction {
                    AuthDirection::Outbound => {
                        if matches!(conn.state, AuthState::OutboundAuthInProgress { .. }) {
                            conn.state = AuthState::Failed("Outbound authentication timed out".to_string());
                        }
                    },
                    AuthDirection::Inbound => {
                        if matches!(conn.state, AuthState::InboundAuthInProgress { .. }) {
                            conn.state = AuthState::Failed("Inbound authentication timed out".to_string());
                        }
                    },
                    AuthDirection::Both => {
                        conn.state = AuthState::Failed("Authentication timed out in both directions".to_string());
                    },
                }
                
                // Generate timeout event
                self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::AuthTimeout {
                    peer_id,
                    address: conn.address.clone(),
                    direction: direction.clone(),
                }));
                
                println!("Authentication timeout for peer {:?}: {:?}", peer_id, direction);
            }
        }
    }

    // Get current authentication state for a peer
    pub fn is_peer_authenticated(&self, peer_id: &PeerId) -> bool {
        if let Some(conn) = self.connections.get(peer_id) {
            matches!(conn.state, AuthState::FullyAuthenticated(_))
        } else {
            false
        }
    }

    // Get peer's authentication metadata if available
    pub fn get_peer_metadata(&self, peer_id: &PeerId) -> Option<HashMap<String, String>> {
        if let Some(conn) = self.connections.get(peer_id) {
            match &conn.state {
                AuthState::FullyAuthenticated(metadata) => Some(metadata.clone()),
                AuthState::OutboundAuthSuccessful(metadata) => Some(metadata.clone()),
                _ => None,
            }
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
                self.connections.insert(peer, ConnectionData {
                    address: remote_addr.clone(),
                    state: AuthState::NotAuthenticated,
                    last_activity: Instant::now(),
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
                println!("Established outbound connection with peer: {:?}", peer);
                
                // Store the connection for our tracking
                self.connections.insert(peer, ConnectionData {
                    address: addr.clone(),
                    state: AuthState::NotAuthenticated,
                    last_activity: Instant::now(),
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
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Pass through to the underlying request_response behaviour
        self.request_response.on_swarm_event(event.clone());

        match event {
            FromSwarm::ConnectionEstablished(connection_established) => {
                println!("Connection established with peer: {:?}", connection_established.peer_id);
                
                // Store the connection if not already stored
                if !self.connections.contains_key(&connection_established.peer_id) {
                    self.connections.insert(
                        connection_established.peer_id,
                        ConnectionData {
                            address: connection_established.endpoint.get_remote_address().clone(),
                            state: AuthState::NotAuthenticated,
                            last_activity: Instant::now(),
                        },
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
                    message: request_response::Message::Request { request, channel, .. },
                    peer,
                    ..
                }) => {
                    // Handle incoming auth request
                    self.handle_auth_request(peer, request, channel);
                    
                    // Continue processing events
                }
                ToSwarm::GenerateEvent(request_response::Event::Message { 
                    message: request_response::Message::Response { response, .. },
                    peer,
                    ..
                }) => {
                    // Handle auth response
                    self.handle_auth_response(peer, response);
                    
                    // Continue processing events
                }
                ToSwarm::GenerateEvent(request_response::Event::OutboundFailure { peer, error, .. }) => {
                    println!("Outbound failure for peer {:?}: {:?}", peer, error);
                    
                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.state = AuthState::Failed(format!("Outbound request failed: {:?}", error));
                        conn.last_activity = Instant::now();
                        
                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::InboundAuthFailure {
                            peer_id: peer,
                            address,
                            reason: format!("Outbound request failed: {:?}", error),
                        }));
                    }
                }
                ToSwarm::GenerateEvent(request_response::Event::InboundFailure { peer, error, .. }) => {
                    println!("Inbound failure for peer {:?}: {:?}", peer, error);
                    
                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.state = AuthState::Failed(format!("Inbound request failed: {:?}", error));
                        conn.last_activity = Instant::now();
                        
                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(PorAuthEvent::OutboundAuthFailure {
                            peer_id: peer,
                            address,
                            reason: format!("Inbound request failed: {:?}", error),
                        }));
                    }
                }
                ToSwarm::NotifyHandler { peer_id, handler, event } => {
                    // Pass events to handler
                    return Poll::Ready(ToSwarm::NotifyHandler {
                        peer_id,
                        handler,
                        event,
                    });
                }
                _ => {
                    // Other events can be processed if needed
                }
            }
        }
        
        // Process any pending events
        if let Some(event) = self.pending_events.pop_front() {
            match event {
                ToSwarm::GenerateEvent(evt) => return Poll::Ready(ToSwarm::GenerateEvent(evt)),
                _ => {}
            }
        }
        
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::futures::stream::StreamExt;
    use libp2p::{
        core::{
            transport,
            upgrade,
        },
        identity,
        noise,
        swarm::{Swarm, SwarmEvent},
        yamux,
        PeerId,
        Transport,
    };
    use std::{collections::HashMap, time::Duration};
    use super::super::por::por::{ProofOfRepresentation, PorUtils};

    // Create test in-memory transport
    fn create_test_transport(keypair: &identity::Keypair) -> libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)> {
        let noise_config = noise::Config::new(keypair).expect("Failed to create noise config");

        transport::MemoryTransport::default()
            .upgrade(upgrade::Version::V1)
            .authenticate(noise_config)
            .multiplex(yamux::Config::default())
            .boxed()
    }

    // Create test swarm with PorAuth behavior
    fn create_test_swarm(keypair: identity::Keypair, por: ProofOfRepresentation) -> Swarm<PorAuthBehaviour> {
        let peer_id = PeerId::from(keypair.public());
        let transport = create_test_transport(&keypair);
        
        // Create basic auth data
        let mut metadata = HashMap::new();
        metadata.insert("node_id".to_string(), peer_id.to_string());
        
        let auth_behaviour = PorAuthBehaviour::with_metadata(por, metadata);
        
        Swarm::new(
            transport,
            auth_behaviour,
            peer_id,
            libp2p::swarm::Config::with_tokio_executor()
        )
    }

    #[tokio::test]
    async fn test_successful_mutual_auth() {
        // Create two nodes with valid PoR data
        let owner_keypair1 = PorUtils::generate_owner_keypair();
        let owner_keypair2 = PorUtils::generate_owner_keypair();
        
        let keypair1 = identity::Keypair::generate_ed25519();
        let keypair2 = identity::Keypair::generate_ed25519();
        
        let peer_id1 = PeerId::from(keypair1.public());
        let peer_id2 = PeerId::from(keypair2.public());
        
        // Create valid PoRs for testing
        let por1 = ProofOfRepresentation::create(
            &owner_keypair1, 
            peer_id1, 
            Duration::from_secs(3600)
        ).expect("Failed to create PoR");
        
        let por2 = ProofOfRepresentation::create(
            &owner_keypair2, 
            peer_id2, 
            Duration::from_secs(3600)
        ).expect("Failed to create PoR");
        
        let mut swarm1 = create_test_swarm(keypair1, por1);
        let mut swarm2 = create_test_swarm(keypair2, por2);
        
        // Set up listener on swarm1
        swarm1.listen_on("/memory/1".parse().unwrap()).unwrap();
        
        // Wait for swarm1 to start listening
        let listen_addr = loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    break address;
                }
                _ => {}
            }
        };
        
        // Connect swarm2 to swarm1
        swarm2.dial(listen_addr).unwrap();
        
        // Process events
        let mut swarm1_verify_event_received = false;
        let mut swarm1_auth_success = false;
        let mut swarm2_verify_event_received = false;
        let mut swarm2_auth_success = false;
        
        let timeout = Duration::from_secs(5);
        let start_time = std::time::Instant::now();
        
        // Event processing loop
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::Behaviour(PorAuthEvent::VerifyPorRequest { 
                        peer_id, por, response_channel, .. 
                    }) = event {
                        assert_eq!(peer_id, peer_id2);
                        // Validate the PoR
                        let validation_result = por.validate();
                        assert!(validation_result.is_ok());
                        swarm1_verify_event_received = true;
                        
                        // Approve authentication
                        swarm1.behaviour_mut().submit_por_verification_result(
                            peer_id,
                            AuthResult::Ok(HashMap::new()),
                            response_channel
                        );
                    } else if let SwarmEvent::Behaviour(PorAuthEvent::MutualAuthSuccess { peer_id, .. }) = event {
                        assert_eq!(peer_id, peer_id2);
                        swarm1_auth_success = true;
                    }
                }
                event = swarm2.select_next_some() => {
                    if let SwarmEvent::Behaviour(PorAuthEvent::VerifyPorRequest { 
                        peer_id, por, response_channel, .. 
                    }) = event {
                        assert_eq!(peer_id, peer_id1);
                        // Validate the PoR
                        let validation_result = por.validate();
                        assert!(validation_result.is_ok());
                        swarm2_verify_event_received = true;
                        
                        // Approve authentication
                        swarm2.behaviour_mut().submit_por_verification_result(
                            peer_id,
                            AuthResult::Ok(HashMap::new()),
                            response_channel
                        );
                    } else if let SwarmEvent::Behaviour(PorAuthEvent::MutualAuthSuccess { peer_id, .. }) = event {
                        assert_eq!(peer_id, peer_id1);
                        swarm2_auth_success = true;
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Check timeout
                    if start_time.elapsed() > timeout {
                        break;
                    }
                }
            }
            
            // If all events received, exit loop
            if swarm1_verify_event_received && swarm1_auth_success && 
               swarm2_verify_event_received && swarm2_auth_success {
                break;
            }
        }
        
        // Verify all expected events occurred
        assert!(swarm1_verify_event_received, "Swarm1 should receive verify request");
        assert!(swarm1_auth_success, "Swarm1 should complete authentication");
        assert!(swarm2_verify_event_received, "Swarm2 should receive verify request");
        assert!(swarm2_auth_success, "Swarm2 should complete authentication");
    }

    #[tokio::test]
    async fn test_auth_failure() {
        // Create owner keypairs
        let owner_keypair1 = PorUtils::generate_owner_keypair();
        let owner_keypair2 = PorUtils::generate_owner_keypair();
        
        // Create node keypairs
        let keypair1 = identity::Keypair::generate_ed25519();
        let keypair2 = identity::Keypair::generate_ed25519();
        
        let peer_id1 = PeerId::from(keypair1.public());
        let peer_id2 = PeerId::from(keypair2.public());
        
        // Create PoRs for testing
        let por1 = ProofOfRepresentation::create(
            &owner_keypair1, 
            peer_id1, 
            Duration::from_secs(3600)
        ).expect("Failed to create PoR");
        
        // Create a PoR with an already expired duration (will be rejected)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let por2 = ProofOfRepresentation::create_with_times(
            &owner_keypair2,
            peer_id2,
            now - 7200,  // 2 hours ago
            now - 3600,  // 1 hour ago
        ).expect("Failed to create expired PoR");
        
        let mut swarm1 = create_test_swarm(keypair1, por1);
        let mut swarm2 = create_test_swarm(keypair2, por2);
        
        // Set up listener on swarm1
        swarm1.listen_on("/memory/2".parse().unwrap()).unwrap();
        
        // Wait for swarm1 to start listening
        let listen_addr = loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    break address;
                }
                _ => {}
            }
        };
        
        // Connect swarm2 to swarm1
        swarm2.dial(listen_addr).unwrap();
        
        // Track auth failure events
        let mut swarm1_verify_event_received = false;
        let mut swarm1_auth_failure = false;
        let mut swarm2_auth_failure = false;
        
        let timeout = Duration::from_secs(5);
        let start_time = std::time::Instant::now();
        
        // Event processing loop
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    println!("Swarm1 event: {:?}", event);
                    if let SwarmEvent::Behaviour(PorAuthEvent::VerifyPorRequest { 
                        peer_id, por, response_channel, .. 
                    }) = event {
                        assert_eq!(peer_id, peer_id2);
                        // Validate the PoR - should fail because it's expired
                        let validation_result = por.validate();
                        assert!(validation_result.is_err());
                        swarm1_verify_event_received = true;
                        
                        // Reject authentication due to expired PoR
                        swarm1.behaviour_mut().submit_por_verification_result(
                            peer_id,
                            AuthResult::Error("Invalid or expired PoR".to_string()),
                            response_channel
                        );
                    } else if let SwarmEvent::Behaviour(PorAuthEvent::OutboundAuthFailure { peer_id, .. }) = event {
                        assert_eq!(peer_id, peer_id2);
                        swarm1_auth_failure = true;
                    }
                }
                event = swarm2.select_next_some() => {
                    println!("Swarm2 event: {:?}", event);
                    if let SwarmEvent::Behaviour(PorAuthEvent::InboundAuthFailure { peer_id, .. }) = event {
                        assert_eq!(peer_id, peer_id1);
                        swarm2_auth_failure = true;
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Check timeout
                    if start_time.elapsed() > timeout {
                        break;
                    }
                }
            }
            
            // If all expected failure events received, exit loop
            if swarm1_verify_event_received && swarm1_auth_failure && swarm2_auth_failure {
                break;
            }
        }
        
        // Verify expected failure events occurred
        assert!(swarm1_verify_event_received, "Swarm1 should receive verify request");
        assert!(swarm1_auth_failure, "Swarm1 should reject authentication");
        assert!(swarm2_auth_failure, "Swarm2 should receive auth failure");
    }
    
    #[tokio::test]
    async fn test_auth_timeout() {
        // Create owner keypair and node keypair for the authenticated node
        let owner_keypair1 = PorUtils::generate_owner_keypair();
        let keypair1 = identity::Keypair::generate_ed25519();
        let keypair2 = identity::Keypair::generate_ed25519();
        
        let peer_id1 = PeerId::from(keypair1.public());
        let peer_id2 = PeerId::from(keypair2.public());
        
        // Create valid PoR
        let por1 = ProofOfRepresentation::create(
            &owner_keypair1, 
            peer_id1, 
            Duration::from_secs(3600)
        ).expect("Failed to create PoR");
        
        // Create a swarm that will respond to auth requests
        let mut swarm1 = create_test_swarm(keypair1, por1);
        
        // Create a test transport
        let transport2 = create_test_transport(&keypair2);
        
        // Create a dummy behavior that won't respond to auth requests
        struct DummyBehaviour;
        
        impl NetworkBehaviour for DummyBehaviour {
            type ConnectionHandler = libp2p::swarm::dummy::ConnectionHandler;
            type ToSwarm = void::Void;
            
            fn handle_established_inbound_connection(
                &mut self,
                _: ConnectionId,
                _: PeerId,
                _: &Multiaddr,
                _: &Multiaddr,
            ) -> Result<THandler<Self>, ConnectionDenied> {
                Ok(libp2p::swarm::dummy::ConnectionHandler)
            }
            
            fn handle_established_outbound_connection(
                &mut self,
                _: ConnectionId,
                _: PeerId,
                _: &Multiaddr,
                _: Endpoint,
                _: libp2p::core::transport::PortUse,
            ) -> Result<THandler<Self>, ConnectionDenied> {
                Ok(libp2p::swarm::dummy::ConnectionHandler)
            }
            
            fn on_connection_handler_event(
                &mut self,
                _: PeerId,
                _: ConnectionId,
                _: THandlerOutEvent<Self>,
            ) {}
            
            fn on_swarm_event(&mut self, _: FromSwarm) {}
            
            fn poll(
                &mut self,
                _: &mut Context<'_>,
            ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
                Poll::Pending
            }
        }
        
        let mut dummy_swarm = Swarm::new(
            transport2,
            DummyBehaviour,
            peer_id2,
            libp2p::swarm::Config::with_tokio_executor()
        );
        
        // Set up listener on swarm1
        swarm1.listen_on("/memory/3".parse().unwrap()).unwrap();
        
        // Wait for swarm1 to start listening
        let listen_addr = loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    break address;
                }
                _ => {}
            }
        };
        
        // Connect dummy swarm to swarm1
        dummy_swarm.dial(listen_addr).unwrap();
        
        // Track timeout events
        let mut timeout_detected = false;
        
        let timeout = Duration::from_secs(10); // Longer timeout for the test itself
        let start_time = std::time::Instant::now();
        
        // Event processing loop
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    println!("Event: {:?}", event);
                    if let SwarmEvent::Behaviour(PorAuthEvent::AuthTimeout { peer_id, direction, .. }) = event {
                        assert_eq!(peer_id, peer_id2);
                        // We accept any direction of timeout
                        timeout_detected = true;
                        println!("Auth timeout detected: {:?}", direction);
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Check test timeout
                    if start_time.elapsed() > timeout {
                        break;
                    }
                }
            }
            
            if timeout_detected {
                break;
            }
        }
        
        // Verify timeout was detected
        assert!(timeout_detected, "Authentication timeout should be detected");
    }
}
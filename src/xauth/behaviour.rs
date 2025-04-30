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
    // Авторизация прошла в обе стороны успешно
    MutualAuthSuccess {
        peer_id: PeerId,
        address: Multiaddr,
    },
    // Мы авторизовали удаленный узел
    OutboundAuthSuccess {
        peer_id: PeerId,
        address: Multiaddr,
    },
    // Удаленный узел авторизовал нас
    InboundAuthSuccess {
        peer_id: PeerId,
        address: Multiaddr,
    },
    // Мы отказали в авторизации удаленному узлу
    OutboundAuthFailure {
        peer_id: PeerId,
        address: Multiaddr,
        reason: String,
    },
    // Удаленный узел отказал нам в авторизации
    InboundAuthFailure {
        peer_id: PeerId,
        address: Multiaddr,
        reason: String,
    },
    // Таймаут авторизации
    AuthTimeout {
        peer_id: PeerId,
        address: Multiaddr,
        direction: AuthDirection,
    },
}

#[derive(Debug, Clone)]
pub enum AuthDirection {
    Inbound,    // От удаленного узла к нам
    Outbound,   // От нас к удаленному узлу
    Both,       // В обоих направлениях
}

// Authentication state for a connection
#[derive(Debug, Clone, PartialEq)]
enum AuthState {
    NotAuthenticated,
    // Ожидаем ответ на наш запрос авторизации
    OutboundAuthInProgress { started: Instant }, 
    // Ожидаем входящий запрос авторизации
    InboundAuthInProgress { started: Instant },
    // Мы авторизовали удаленный узел, но он еще не авторизовал нас
    OutboundAuthSuccessful,
    // Удаленный узел авторизовал нас, но мы еще не авторизовали его
    InboundAuthSuccessful,
    // Полная взаимная авторизация
    FullyAuthenticated,
    // Ошибка авторизации
    Failed(String),
}

// Connection data structure to track auth state
struct ConnectionData {
    address: Multiaddr,
    state: AuthState,
    // Время последней активности по этому соединению
    last_activity: Instant,
}

// Define the behaviour
pub struct XAuthBehaviour {
    // Using cbor codec for request-response
    pub request_response: request_response::cbor::Behaviour<AuthRequest, AuthResponse>,
    
    // Additional state for tracking connections
    connections: HashMap<PeerId, ConnectionData>,
    
    // Events to be emitted
    pending_events: VecDeque<ToSwarm<XAuthEvent, request_response::OutboundRequestId>>,
    
    // Данные для авторизации
    auth_data: HashMap<String, String>,
}

impl XAuthBehaviour {
    pub fn new() -> Self {
        // Настройка данных для авторизации по умолчанию
        let mut auth_data = HashMap::new();
        auth_data.insert("hello".to_string(), "world".to_string());
        
        Self::with_auth_data(auth_data)
    }
    
    pub fn with_auth_data(auth_data: HashMap<String, String>) -> Self {
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
            auth_data,
        }
    }

    // Handle incoming authentication request
    fn handle_auth_request(&mut self, peer_id: PeerId, data: HashMap<String, String>, channel: ResponseChannel<AuthResponse>) {
        println!("Processing auth request from {:?}: {:?}", peer_id, data);
        
        // Проверка авторизационных данных
        let result = if data.get("hello") == Some(&"world".to_string()) {
            AuthResult::Ok
        } else {
            AuthResult::Error("Invalid authentication data".to_string())
        };

        // Отправляем ответ сразу
        if let Err(e) = self.request_response.send_response(channel, AuthResponse(result.clone())) {
            println!("Failed to send auth response: {:?}", e);
            return;
        }

        // Обновляем состояние соединения
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.last_activity = Instant::now();
            
            match &result {
                AuthResult::Ok => {
                    // Обновляем состояние в зависимости от текущего статуса
                    match conn.state {
                        AuthState::NotAuthenticated | AuthState::InboundAuthInProgress { .. } => {
                            conn.state = AuthState::InboundAuthSuccessful;
                            
                            // Генерируем событие успешной входящей авторизации
                            self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::InboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                            
                            // Если мы еще не отправили запрос на авторизацию, отправляем его
                            if matches!(conn.state, AuthState::InboundAuthSuccessful) {
                                self.start_outbound_auth(peer_id);
                            }
                        }
                        AuthState::OutboundAuthSuccessful => {
                            // Теперь у нас полная взаимная авторизация
                            conn.state = AuthState::FullyAuthenticated;
                            
                            // Генерируем событие полной авторизации
                            self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::MutualAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                        }
                        _ => {
                            // Другие состояния не должны тут встречаться, но если все же
                            // встречаются, устанавливаем InboundAuthSuccessful
                            conn.state = AuthState::InboundAuthSuccessful;
                            
                            self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::InboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                        }
                    }
                }
                AuthResult::Error(reason) => {
                    conn.state = AuthState::Failed(reason.clone());
                    
                    // Генерируем событие неудачной исходящей авторизации
                    self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::OutboundAuthFailure {
                        peer_id,
                        address: conn.address.clone(),
                        reason: reason.clone(),
                    }));
                }
            }
        }
    }

    // Handle authentication response
    fn handle_auth_response(&mut self, peer_id: PeerId, result: AuthResult) {
        println!("Received auth response from {:?}: {:?}", peer_id, result);
        
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            conn.last_activity = Instant::now();
            
            match result {
                AuthResult::Ok => {
                    // Обновляем состояние в зависимости от текущего
                    match conn.state {
                        AuthState::OutboundAuthInProgress { .. } => {
                            conn.state = AuthState::OutboundAuthSuccessful;
                            
                            // Генерируем событие успешной исходящей авторизации
                            self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::OutboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                        }
                        AuthState::InboundAuthSuccessful => {
                            // Теперь у нас полная взаимная авторизация
                            conn.state = AuthState::FullyAuthenticated;
                            
                            // Генерируем событие полной авторизации
                            self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::MutualAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                        }
                        _ => {
                            // Другие состояния не должны тут встречаться, но если все же
                            // встречаются, устанавливаем OutboundAuthSuccessful
                            conn.state = AuthState::OutboundAuthSuccessful;
                            
                            self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::OutboundAuthSuccess {
                                peer_id,
                                address: conn.address.clone(),
                            }));
                        }
                    }
                }
                AuthResult::Error(reason) => {
                    conn.state = AuthState::Failed(reason.clone());
                    
                    // Генерируем событие неудачной входящей авторизации
                    self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::InboundAuthFailure {
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
            // Обновляем состояние
            conn.state = AuthState::OutboundAuthInProgress { started: Instant::now() };
            conn.last_activity = Instant::now();
            
            println!("Starting outbound auth with peer {:?}", peer_id);
            
            // Отправляем запрос аутентификации
            self.request_response.send_request(&peer_id, AuthRequest(self.auth_data.clone()));
        }
    }

    // Start inbound authentication waiting
    fn start_inbound_auth_waiting(&mut self, peer_id: PeerId) {
        if let Some(conn) = self.connections.get_mut(&peer_id) {
            // Устанавливаем состояние ожидания входящей авторизации
            conn.state = AuthState::InboundAuthInProgress { started: Instant::now() };
            conn.last_activity = Instant::now();
            
            println!("Waiting for inbound auth from peer {:?}", peer_id);
        }
    }

    // Start authentication in both directions
    pub fn start_authentication(&mut self, peer_id: &PeerId) {
        // Запускаем исходящую авторизацию
        self.start_outbound_auth(*peer_id);
        
        // Начинаем ожидать входящую авторизацию
        self.start_inbound_auth_waiting(*peer_id);
    }

    // Check for authentication timeouts
    fn check_timeouts(&mut self) {
        let now = Instant::now();
        
        // Ищем соединения с таймаутами
        let timed_out_peers: Vec<(PeerId, AuthDirection)> = self.connections.iter()
            .filter_map(|(peer_id, conn)| {
                match conn.state {
                    // Исходящая авторизация с таймаутом
                    AuthState::OutboundAuthInProgress { started } 
                        if now.duration_since(started) > AUTH_TIMEOUT => {
                        Some((*peer_id, AuthDirection::Outbound))
                    },
                    // Входящая авторизация с таймаутом
                    AuthState::InboundAuthInProgress { started }
                        if now.duration_since(started) > AUTH_TIMEOUT => {
                        Some((*peer_id, AuthDirection::Inbound))
                    },
                    // Время неактивности превысило таймаут
                    _ if now.duration_since(conn.last_activity) > AUTH_TIMEOUT * 2 => {
                        Some((*peer_id, AuthDirection::Both))
                    },
                    _ => None,
                }
            })
            .collect();

        // Обрабатываем таймауты
        for (peer_id, direction) in timed_out_peers {
            if let Some(conn) = self.connections.get_mut(&peer_id) {
                // Обновляем состояние соединения
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
                
                // Генерируем событие таймаута
                self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::AuthTimeout {
                    peer_id,
                    address: conn.address.clone(),
                    direction: direction.clone(),
                }));
                
                println!("Authentication timeout for peer {:?}: {:?}", peer_id, direction);
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
        
        // Примечание: обработка событий теперь происходит в методе poll()
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
                
                // Begin authentication process on connection establishment
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
                    // Обрабатываем входящий запрос авторизации непосредственно здесь
                    self.handle_auth_request(peer, request.0, channel);
                    
                    // Продолжаем обработку событий, поэтому не возвращаем Poll::Ready
                }
                ToSwarm::GenerateEvent(request_response::Event::Message { 
                    message: request_response::Message::Response { response, .. },
                    peer,
                    ..
                }) => {
                    // Обрабатываем ответ на наш запрос авторизации
                    self.handle_auth_response(peer, response.0);
                    
                    // Продолжаем обработку событий
                }
                ToSwarm::GenerateEvent(request_response::Event::OutboundFailure { peer, error, .. }) => {
                    println!("Outbound failure for peer {:?}: {:?}", peer, error);
                    
                    // Mark connection as failed if it exists
                    if let Some(conn) = self.connections.get_mut(&peer) {
                        conn.state = AuthState::Failed(format!("Outbound request failed: {:?}", error));
                        conn.last_activity = Instant::now();
                        
                        let address = conn.address.clone();
                        self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::InboundAuthFailure {
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
                        self.pending_events.push_back(ToSwarm::GenerateEvent(XAuthEvent::OutboundAuthFailure {
                            peer_id: peer,
                            address,
                            reason: format!("Inbound request failed: {:?}", error),
                        }));
                    }
                }
                ToSwarm::NotifyHandler { peer_id, handler, event } => {
                    // Передаем события обработчику
                    return Poll::Ready(ToSwarm::NotifyHandler {
                        peer_id,
                        handler,
                        event,
                    });
                }
                _ => {
                    // Other events are passed through - при необходимости можно обрабатывать
                }
            }
        }
        
        // Then, process any events we have
        if let Some(event) = self.pending_events.pop_front() {
            // Отправляем событие в swarm
            match event {
                ToSwarm::GenerateEvent(evt) => return Poll::Ready(ToSwarm::GenerateEvent(evt)),
                _ => {}
            }
        }
        
        Poll::Pending
    }
}
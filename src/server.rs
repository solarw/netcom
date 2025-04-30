use crate::codec::EchoCodec;
use crate::protocol::{
    create_protocol, create_request_response_config, EchoRequest, EchoResponse, PROTOCOL_ID
};
use crate::xauth::behaviour::{AuthResponse, AuthResult, XAuthBehaviour, XAuthEvent};
use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    identity,
    identify,
    multiaddr::Protocol,
    request_response::{
        self, ProtocolSupport,
    },
    swarm::{NetworkBehaviour, SwarmEvent},
    quic, PeerId, Multiaddr
};
use std::time::Duration;
use std::{
    fs,
    path::PathBuf,
    str,
};

// Определяем поведение узла
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ServerBehaviourEvent")]
pub struct ServerBehaviour {
    request_response: request_response::Behaviour<EchoCodec>,
    identify: identify::Behaviour,
    xauth: XAuthBehaviour,
}

#[derive(Debug)]
pub enum ServerBehaviourEvent {
    RequestResponse(request_response::Event<EchoRequest, EchoResponse>),
    Identify(identify::Event),
    XAuth(XAuthEvent),
}

impl From<request_response::Event<EchoRequest, EchoResponse>> for ServerBehaviourEvent {
    fn from(event: request_response::Event<EchoRequest, EchoResponse>) -> Self {
        ServerBehaviourEvent::RequestResponse(event)
    }
}

impl From<identify::Event> for ServerBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        ServerBehaviourEvent::Identify(event)
    }
}

impl From<XAuthEvent> for ServerBehaviourEvent {
    fn from(event: XAuthEvent) -> Self {
        ServerBehaviourEvent::XAuth(event)
    }
}

pub async fn run_server(key_file: Option<PathBuf>) -> Result<()> {
    // Загружаем или создаем ключ
    let keypair = match key_file.as_ref() {
        Some(path) if path.exists() => {
            let key_bytes = fs::read(path)?;
            identity::Keypair::from_protobuf_encoding(&key_bytes)?
        }
        _ => {
            let keypair = identity::Keypair::generate_ed25519();
            
            if let Some(path) = key_file {
                // Сохраняем ключ, если указан путь
                let key_bytes = keypair.to_protobuf_encoding()?;
                fs::write(path, key_bytes)?;
            }
            
            keypair
        }
    };

    // Получаем ID узла из ключа
    let peer_id = PeerId::from(keypair.public());
    println!("Server PeerID: {}", peer_id);

 

    // Создаем поведение
    let protocol = create_protocol();
    let config = create_request_response_config();

    // Теперь протоколы передаются непосредственно в Behaviour::new
    let request_response = request_response::Behaviour::new(
        vec![(protocol, ProtocolSupport::Full)],
        config,
    );

    let identify = identify::Behaviour::new(identify::Config::new(
        PROTOCOL_ID.to_string(),
        keypair.public(),
    ));

    let xauth = XAuthBehaviour::new();
    let behaviour = ServerBehaviour {
        request_response,
        identify,
        xauth,
    };

    // Создаем сваму
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| behaviour).unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build();

    // Слушаем на всех интерфейсах
    let listen_addr = "/ip4/0.0.0.0/udp/0/quic-v1".parse::<Multiaddr>()?;
    swarm.listen_on(listen_addr)?;
    
    // Основной цикл обработки событий
    loop {
        match swarm.select_next_some().await {

            SwarmEvent::NewListenAddr { address, .. } => {
                let p2p_addr = address.clone().with(Protocol::P2p(peer_id));
                println!("Слушаем на адресе: {}", address);
                println!("URI для подключения: {}", p2p_addr);
            }
            SwarmEvent::Behaviour(event) => match event {
                ServerBehaviourEvent::RequestResponse(event) => match event {
                    request_response::Event::Message {connection_id,  peer, message } => {
                        match message {
                            request_response::Message::Request {
                                request, channel, ..
                            } => {
                                let EchoRequest(data) = request;
                                
                                if let Ok(text) = str::from_utf8(&data) {
                                    println!("Получено от клиента {}: {}", peer, text);
                                    
                                    // Создаем ответ с префиксом
                                    let response_text = format!("ответ от сервера: {}", text);
                                    let response = EchoResponse(response_text.into_bytes());
                                    
                                    // Отправляем ответ
                                    if let Err(e) = swarm.behaviour_mut().request_response.send_response(channel, response) {
                                        eprintln!("Ошибка при отправке ответа: {:?}", e);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        println!("Ответ отправлен узлу: {}", peer);
                    }
                    _ => {}
                },
                ServerBehaviourEvent::Identify(event) => {
                    println!("Identify событие: {:?}", event);
                }
                ServerBehaviourEvent::XAuth(event) => {
                    match event {
                        XAuthEvent::InboundRequest { request, peer, channel } => {
                            println!("Received auth request from {:?}: {:?}", peer, request);

                            // Process the authentication request
                            let result = if request.get("hello") == Some(&"world".to_string()) {
                                println!("Authentication successful for {:?}", peer);
                                AuthResult::Ok
                            } else {
                                println!("Authentication failed for {:?}", peer);
                                AuthResult::Error("Invalid authentication data".to_string())
                            };

                            // Send response
                            if let Err(e) = swarm.behaviour_mut().xauth.request_response.send_response(channel, AuthResponse(result)) {
                                println!("Failed to send response: {:?}", e);
                            }
                        }
                        XAuthEvent::Success { peer_id, .. } => {
                            println!("Authentication successful for peer: {:?}", peer_id);
                        }
                        XAuthEvent::Failure { peer_id, reason, .. } => {
                            println!("Authentication failed for peer: {:?}, reason: {}", peer_id, reason);
                        }
                        XAuthEvent::Timeout { peer_id, .. } => {
                            println!("Authentication timed out for peer: {:?}", peer_id);
                        }
                    }
                    
                }
            },
            _ => {}
        }
    }
}
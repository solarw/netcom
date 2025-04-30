use crate::codec::EchoCodec;
use crate::protocol::{
    create_protocol, create_request_response_config, EchoRequest, EchoResponse, PROTOCOL_ID,
};
use crate::xauth::behaviour::{AuthResponse, AuthResult, XAuthBehaviour, XAuthEvent};
use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use libp2p::{
    identify, identity,
    multiaddr::Protocol,
    quic,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId,
};
use std::time::Duration;
use std::{
    io::{self, Write},
    str, thread,
};

// Определяем поведение узла-клиента
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ClientBehaviourEvent")]
pub struct ClientBehaviour {
    request_response: request_response::Behaviour<EchoCodec>,
    identify: identify::Behaviour,
    xauth: XAuthBehaviour,
}

#[derive(Debug)]
pub enum ClientBehaviourEvent {
    RequestResponse(request_response::Event<EchoRequest, EchoResponse>),
    Identify(identify::Event),
    XAuth(XAuthEvent),
}

impl From<request_response::Event<EchoRequest, EchoResponse>> for ClientBehaviourEvent {
    fn from(event: request_response::Event<EchoRequest, EchoResponse>) -> Self {
        ClientBehaviourEvent::RequestResponse(event)
    }
}

impl From<identify::Event> for ClientBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        ClientBehaviourEvent::Identify(event)
    }
}

impl From<XAuthEvent> for ClientBehaviourEvent {
    fn from(event: XAuthEvent) -> Self {
        ClientBehaviourEvent::XAuth(event)
    }
}
pub async fn run_client(server_uri: &str) -> Result<()> {
    // Создаем случайный ключ для клиента
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    println!("Client PeerID: {}", peer_id);

    // Парсим URI сервера
    let server_addr = server_uri.parse::<Multiaddr>()?;

    println!("Подключаемся к серверу: {}", server_uri);

    // Создаем протокол и конфигурацию
    let protocol = create_protocol();
    let config = create_request_response_config();

    // Создаем поведение
    // Протокол и конфиг передаются как готовый итератор, в этой версии API
    let request_response =
        request_response::Behaviour::new(vec![(protocol, ProtocolSupport::Full)], config);

    let identify = identify::Behaviour::new(identify::Config::new(
        PROTOCOL_ID.to_string(),
        keypair.public(),
    ));
    let xauth = XAuthBehaviour::new();
    let behaviour = ClientBehaviour {
        request_response,
        identify,
        xauth,
    };

    // Создаем сваму
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| behaviour)
        .unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build();

    // Начинаем подключение к серверу
    swarm.dial(server_addr.clone())?;
    println!("Подключение к серверу инициировано...");

    // Флаг, указывающий на то, что мы подключены и готовы отправлять сообщения
    let mut connected = false;

    // Обработка ввода пользователя в отдельном потоке с использованием стандартного thread вместо tokio::spawn
    let (tx, mut rx) = futures::channel::mpsc::channel(1);

    // Используем обычный поток вместо tokio::spawn, чтобы избежать ошибки Send для StdinLock
    let tx_clone = tx.clone();
    thread::spawn(move || {
        let mut buffer = String::new();
        let stdin = io::stdin();

        loop {
            buffer.clear();
            print!("> ");
            io::stdout().flush().unwrap();

            match stdin.read_line(&mut buffer) {
                Ok(_) => {
                    let message = buffer.trim().to_owned();
                    if !message.is_empty() {
                        // Блокирующий вызов отправки сообщения через канал
                        if futures::executor::block_on(tx_clone.clone().send(message)).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Ошибка чтения ввода: {:?}", e);
                    break;
                }
            }
        }
    });

    // Основной цикл обработки событий
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            println!("Подключение к серверу установлено!");
                            connected = true;
                            println!("Введите сообщение для отправки серверу:");
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            println!("Соединение с сервером закрыто");
                            // Не устанавливаем connected = false здесь, так как мы выходим из функции
                            return Ok(());
                    }
                    SwarmEvent::Behaviour(event) => match event {
                        ClientBehaviourEvent::RequestResponse(event) => match event {
                            request_response::Event::Message { connection_id, peer, message } => {
                                match message {
                                    request_response::Message::Response { response, .. } => {
                                        let EchoResponse(data) = response;
                                        if let Ok(text) = str::from_utf8(&data) {
                                            println!("Получен ответ от сервера {}: {}", peer, text);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            request_response::Event::OutboundFailure { peer, error, .. } => {
                                eprintln!("Ошибка исходящего запроса к узлу {}: {:?}", peer, error);
                            }
                            request_response::Event::InboundFailure { peer, error, .. } => {
                                eprintln!("Ошибка входящего запроса от узла {}: {:?}", peer, error);
                            }
                            _ => {}
                        },
                        ClientBehaviourEvent::Identify(event) => {
                            println!("Identify событие: {:?}", event);
                        },
                        ClientBehaviourEvent::XAuth(event) => {
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
            message = rx.next() => {
                if let Some(msg) = message {
                    if connected {
                        println!("Отправка сообщения: {}", msg);
                        // Отправляем сообщение серверу
                        let request = EchoRequest(msg.into_bytes());
                        //swarm.behaviour_mut().request_response.send_request(&server_peer_id, request);
                    } else {
                        println!("Не подключено к серверу, не могу отправить сообщение");
                    }
                }
            }
        }
    }
}

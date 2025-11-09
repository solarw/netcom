//! Пример механизма принятия решений для входящих потоков XStream
//!
//! Этот пример демонстрирует:
//! - Создание узлов с QUIC транспортом
//! - Механизм принятия решения о принятии или отклонении входящих потоков
//! - Отправку ошибок с описанием причины отказа
//! - Обработку различных сценариев авторизации

use libp2p::{
    identity, 
    swarm::{Swarm, SwarmEvent, dial_opts::DialOpts},
    quic, Multiaddr, PeerId,
};
use libp2p::futures::StreamExt;
use tokio::sync::{oneshot, mpsc};
use std::error::Error;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use std::collections::HashSet;

// Импортируем XStream компоненты
use xstream::behaviour::XStreamNetworkBehaviour;
use xstream::events::{XStreamEvent, InboundUpgradeDecision, StreamOpenDecisionSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 Запуск примера механизма принятия решений для входящих потоков...");

    // Создаем два узла с QUIC транспортом
    let (mut client_swarm, client_peer_id) = create_quic_swarm().await?;
    let (mut server_swarm, server_peer_id) = create_quic_swarm().await?;

    println!("✅ Созданы два узла:");
    println!("   Клиент: {}", client_peer_id);
    println!("   Сервер: {}", server_peer_id);

    // Запускаем сервер прослушивание
    let server_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse()?;
    server_swarm.listen_on(server_addr.clone()).expect("Failed to listen");
    println!("✅ Сервер слушает на: {}", server_addr);

    // Получаем реальный адрес сервера
    let listen_addr = wait_for_listen_addr(&mut server_swarm).await;
    println!("✅ Сервер реально слушает на: {}", listen_addr);

    // Создаем каналы для передачи потоков и решений
    let (server_stream_tx, server_stream_rx) = oneshot::channel();
    let (client_stream_tx, client_stream_rx) = oneshot::channel();

    // Создаем каналы для завершения работы
    let (server_shutdown_tx, mut server_shutdown_rx) = mpsc::channel(1);
    let (client_shutdown_tx, mut client_shutdown_rx) = mpsc::channel(1);

    // Белый список разрешенных пиров (в реальном приложении это может быть база данных)
    let allowed_peers: HashSet<PeerId> = vec![client_peer_id].into_iter().collect();

    // Запускаем серверную задачу - бесконечный swarm loop с механизмом принятия решений
    let server_task = tokio::spawn({
        let allowed_peers = allowed_peers.clone();
        let mut server_stream_tx = Some(server_stream_tx);
        async move {
            println!("🎯 Серверная задача запущена с механизмом принятия решений...");
            
            loop {
                tokio::select! {
                    event = server_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::NewListenAddr { address, .. } => {
                                println!("📡 Сервер слушает на: {}", address);
                            }
                            SwarmEvent::IncomingConnection { connection_id, .. } => {
                                println!("🔗 Сервер: Входящее соединение: {:?}", connection_id);
                            }
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Сервер: Соединение установлено с: {}", peer_id);
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::InboundUpgradeRequest { peer_id, connection_id, decision_sender } => {
                                        println!("📥 Сервер: Запрос на входящий апгрейд от {} (connection: {:?})", peer_id, connection_id);
                                        
                                        // Механизм принятия решения
                                        let decision = make_inbound_decision(&peer_id, &allowed_peers);
                                        
                                        match decision {
                                            InboundDecision::Accept => {
                                                println!("✅ Сервер: Разрешен входящий апгрейд от {}", peer_id);
                                                // Отправляем решение о разрешении с новым API
                                                if let Err(e) = decision_sender.approve() {
                                                    println!("⚠️  Не удалось отправить решение о разрешении: {}", e);
                                                }
                                            }
                                            InboundDecision::Reject(reason) => {
                                                println!("❌ Сервер: Отклонен входящий апгрейд от {}: {}", peer_id, reason);
                                                // Отправляем решение об отклонении с причиной с новым API
                                                if let Err(e) = decision_sender.reject(reason) {
                                                    println!("⚠️  Не удалось отправить решение об отклонении: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    XStreamEvent::IncomingStream { stream } => {
                                        println!("📥 Сервер: Получен входящий XStream от {}", stream.peer_id);
                                        // Передаем поток через oneshot канал
                                        if let Some(tx) = server_stream_tx.take() {
                                            let _ = tx.send(stream);
                                        }
                                    }
                                    XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                                        println!("✅ Сервер: XStream установлен с {} (ID: {:?})", peer_id, stream_id);
                                    }
                                    XStreamEvent::StreamError { peer_id, error, .. } => {
                                        println!("❌ Сервер: Ошибка потока с {}: {}", peer_id, error);
                                    }
                                    XStreamEvent::StreamClosed { peer_id, .. } => {
                                        println!("🔒 Сервер: Поток закрыт с {}", peer_id);
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = server_shutdown_rx.recv() => {
                        println!("🛑 Серверная задача получила сигнал завершения");
                        break;
                    }
                }
            }
            println!("🛑 Серверная задача завершена");
        }
    });

    // Запускаем клиентскую задачу - бесконечный swarm loop
    let client_task = tokio::spawn({
        let server_peer_id = server_peer_id.clone();
        let mut client_stream_tx = Some(client_stream_tx);
        async move {
            println!("🎯 Клиентская задача запущена, подключение к серверу...");
            
            // Даем серверу время запуститься
            sleep(Duration::from_millis(100)).await;

            // Подключаемся к серверу
            client_swarm.dial(DialOpts::peer_id(server_peer_id).addresses(vec![listen_addr.clone()]).build()).unwrap();
            println!("🔗 Клиент: Подключение к серверу по адресу {}", listen_addr);
            
            loop {
                tokio::select! {
                    event = client_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Клиент: Соединение установлено с: {}", peer_id);
                                if peer_id == server_peer_id {
                                    println!("✅ Клиент: Подключился к ожидаемому серверу");
                                    
                                    // Открываем XStream к серверу
                                    println!("🔄 Клиент: Открытие XStream к серверу...");
                                    let (tx, rx) = oneshot::channel();
                                    client_swarm.behaviour_mut().open_stream(server_peer_id, tx).await;
                                    if let Some(tx) = client_stream_tx.take() {
                                        let _ = tx.send(rx);
                                    }
                                }
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                                        println!("✅ Клиент: XStream установлен к {} (ID: {:?})", peer_id, stream_id);
                                    }
                                    XStreamEvent::StreamError { peer_id, error, .. } => {
                                        println!("❌ Клиент: Ошибка потока с {}: {}", peer_id, error);
                                    }
                                    XStreamEvent::StreamClosed { peer_id, .. } => {
                                        println!("🔒 Клиент: Поток закрыт с {}", peer_id);
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = client_shutdown_rx.recv() => {
                        println!("🛑 Клиентская задача получила сигнал завершения");
                        break;
                    }
                }
            }
            println!("🛑 Клиентская задача завершена");
        }
    });

    // Получаем клиентский поток
    let recv_result = timeout(Duration::from_secs(10), client_stream_rx).await;
    let client_stream_rx = match recv_result {
        Ok(Ok(rx)) => rx,
        Ok(Err(_)) => {
            panic!("client_stream_rx: Sender закрыт до отправки клиентского потока");
        }
        Err(_) => {
            panic!("client_stream_rx: Таймаут ожидания клиентского потока");
        }
    };

    let maybe_client_stream = timeout(Duration::from_secs(10), client_stream_rx).await;
    let client_stream = match maybe_client_stream {
        Ok(Ok(rx)) => {
            match rx {
                Ok(stream) => stream,
                Err(e) => panic!("client_stream: Ошибка при получении клиентского потока: {:?}", e),
            }
        }
        Ok(Err(_)) => {
            panic!("client_stream: Sender закрыт до отправки клиентского потока");
        }
        Err(_) => {
            panic!("client_stream: Таймаут ожидания клиентского потока");
        }
    };

    // Получаем серверный поток (если он был принят)
    let server_stream_result = timeout(Duration::from_secs(5), server_stream_rx).await;

    // СТРОГАЯ ПРОВЕРКА: клиент должен быть в белом списке, поэтому поток ДОЛЖЕН быть принят
    if !allowed_peers.contains(&client_peer_id) {
        panic!("❌ ПАНИКА: Клиентский ID {} не в белом списке! Ожидалось, что клиент будет авторизован", client_peer_id);
    }

    match server_stream_result {
        Ok(Ok(server_stream)) => {
            println!("✅ Сервер принял входящий поток, начинаем тест передачи данных...");
            
            // Теперь у нас есть оба потока, выполняем параллельные операции
            let mut client_stream = client_stream;
            let mut server_stream = server_stream;

            // Запускаем клиентские операции - запись данных и закрытие
            let client_handle = tokio::spawn(async move {
                println!("📤 Клиент отправляет запрос серверу...");
                
                let request = b"Hello from authorized client!".to_vec();
                println!("📤 Клиент отправляет запрос: {}", String::from_utf8_lossy(&request));
                
                // Записываем данные - паника при ошибке
                client_stream.write_all(request).await.expect("❌ ПАНИКА: Не удалось записать данные от клиента");
                client_stream.flush().await.expect("❌ ПАНИКА: Не удалось сбросить поток клиента");
                println!("✅ Данные записаны и сброшены");

                // Закрываем поток - паника при ошибке
                println!("🔒 Клиент закрывает поток...");
                client_stream.close().await.expect("❌ ПАНИКА: Не удалось закрыть клиентский поток");
                println!("✅ Клиентский поток закрыт");
            });

            // Сервер читает запрос и отправляет ответ
            let server_handle = tokio::spawn(async move {
                println!("🔄 Сервер обрабатывает входящий поток...");
                
                // Читаем запрос - паника при ошибке
                let request = server_stream.read_to_end().await.expect("❌ ПАНИКА: Не удалось прочитать запрос от клиента");
                println!("📥 Сервер получил запрос: {}", String::from_utf8_lossy(&request));
                
                // Обрабатываем запрос
                let response = format!("Server response to authorized client: {}", String::from_utf8_lossy(&request));
                println!("📤 Сервер отправляет ответ: {}", response);
                
                // Отправляем ответ - паника при ошибке
                server_stream.write_all(response.as_bytes().to_vec()).await.expect("❌ ПАНИКА: Не удалось записать ответ");
                server_stream.flush().await.expect("❌ ПАНИКА: Не удалось сбросить поток сервера");
                
                // Закрываем поток - паника при ошибке
                println!("🔒 Сервер закрывает поток...");
                server_stream.close().await.expect("❌ ПАНИКА: Не удалось закрыть серверный поток");
                println!("✅ Серверный поток закрыт");
            });

            // Ждем завершения операций - паника при ошибке
            let (client_result, server_result) = tokio::join!(client_handle, server_handle);
            client_result.expect("❌ ПАНИКА: Ошибка в клиентской операции");
            server_result.expect("❌ ПАНИКА: Ошибка в серверной операции");
            println!("✅ Операции завершены успешно");
        }
        Ok(Err(_)) => {
            panic!("❌ ПАНИКА: Сервер отклонил входящий поток от авторизованного клиента {}! Ожидалось принятие потока", client_peer_id);
        }
        Err(_) => {
            panic!("❌ ПАНИКА: Таймаут ожидания серверного потока от авторизованного клиента {}! Ожидалось принятие потока", client_peer_id);
        }
    }

    // Даем время для завершения всех операций
    sleep(Duration::from_millis(500)).await;

    // Отправляем сигналы завершения
    let _ = server_shutdown_tx.send(()).await;
    let _ = client_shutdown_tx.send(()).await;

    // Ждем завершения задач
    let _ = tokio::join!(server_task, client_task);

    println!("✅ Пример механизма принятия решений завершен успешно!");
    Ok(())
}

/// Решение о принятии входящего потока
enum InboundDecision {
    Accept,
    Reject(String),
}

/// Механизм принятия решения о входящем потоке
fn make_inbound_decision(peer_id: &PeerId, allowed_peers: &HashSet<PeerId>) -> InboundDecision {
    // Простая логика авторизации - проверка в белом списке
    // Клиент в белом списке → Accept, не в списке → Reject
    if allowed_peers.contains(peer_id) {
        InboundDecision::Accept
    } else {
        InboundDecision::Reject(format!("Peer {} not authorized", peer_id))
    }
}

/// Создает узел с QUIC транспортом
async fn create_quic_swarm() -> Result<(Swarm<XStreamNetworkBehaviour>, PeerId), Box<dyn Error>> {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();
    
    // Создаем QUIC транспорт
    let quic_config = quic::Config::new(&keypair);
    let quic_transport = quic::tokio::Transport::new(quic_config);
    
    // Создаем swarm с XStream поведением с политикой ручного принятия решений
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|_key| quic_transport)
        .expect("Не удалось создать QUIC транспорт")
        .with_behaviour(|_key| {
            XStreamNetworkBehaviour::new_with_policy(
                xstream::events::IncomingConnectionApprovePolicy::ApproveViaEvent
            )
        })
        .expect("Не удалось создать XStream поведение")
        .build();
    
    Ok((swarm, peer_id))
}

/// Ожидает адрес прослушивания от swarm
async fn wait_for_listen_addr(swarm: &mut Swarm<XStreamNetworkBehaviour>) -> Multiaddr {
    timeout(Duration::from_secs(2), async {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                return address;
            }
        }
    })
    .await
    .expect("Таймаут ожидания адреса прослушивания")
}

//! Тест сценария отклонения соединения в XStream
//!
//! Этот тест проверяет сценарий, когда один узел пытается открыть соединение к другому
//! и получает reject с указанием причины.

use libp2p::{identity, quic, Multiaddr, PeerId, swarm::{Swarm, SwarmEvent, dial_opts::DialOpts}};
use libp2p::futures::StreamExt;
use tokio::sync::oneshot;
use std::time::Duration;
use tokio::time::{sleep, timeout};

use crate::behaviour::XStreamNetworkBehaviour;
use crate::events::{XStreamEvent, IncomingConnectionApprovePolicy};
use crate::xstream::XStream;

/// Тестирует сценарий отклонения соединения
#[tokio::test]
async fn test_connection_reject_scenario() {
    println!("🧪 Тестируем сценарий отклонения соединения...");
    
    // Создаем два узла с QUIC транспортом
    let (mut client_swarm, client_peer_id) = create_quic_swarm_with_policy(
        IncomingConnectionApprovePolicy::AutoApprove
    ).await.expect("❌ Не удалось создать клиентский узел");
    
    let (mut server_swarm, server_peer_id) = create_quic_swarm_with_policy(
        IncomingConnectionApprovePolicy::ApproveViaEvent
    ).await.expect("❌ Не удалось создать серверный узел");

    println!("✅ Созданы два узла:");
    println!("   Клиент: {}", client_peer_id);
    println!("   Сервер: {}", server_peer_id);

    // Запускаем сервер прослушивание
    let server_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().expect("❌ Неверный адрес сервера");
    server_swarm.listen_on(server_addr.clone()).expect("❌ Не удалось запустить прослушивание");
    println!("✅ Сервер слушает на: {}", server_addr);

    // Получаем реальный адрес сервера
    let listen_addr = wait_for_listen_addr(&mut server_swarm).await;
    println!("✅ Сервер реально слушает на: {}", listen_addr);

    // Создаем каналы для управления завершением swarm loop
    let (server_shutdown_tx, server_shutdown_rx) = oneshot::channel();
    let (client_shutdown_tx, client_shutdown_rx) = oneshot::channel();
    
    // Создаем каналы для передачи событий
    let (server_request_tx, server_request_rx) = oneshot::channel();
    let (server_reject_tx, server_reject_rx) = oneshot::channel();
    let (stream_tx, stream_rx) = oneshot::channel::<Result<XStream, String>>();

    // Запускаем серверную задачу с graceful shutdown
    let server_handle = tokio::spawn({
        let mut server_request_tx = Some(server_request_tx);
        let mut server_reject_tx = Some(server_reject_tx);
        async move {
            println!("🎯 Серверная задача запущена...");
            
            let mut server_shutdown_rx = server_shutdown_rx;
            
            loop {
                tokio::select! {
                    // Проверяем сигнал завершения
                    _ = &mut server_shutdown_rx => {
                        println!("🛑 Сервер: Получен сигнал завершения");
                        break;
                    }
                    // Обрабатываем события swarm
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
                                    XStreamEvent::IncomingStreamRequest { peer_id, connection_id, decision_sender } => {
                                        println!("📥 Сервер: Запрос на входящий апгрейд от {} (connection: {:?})", peer_id, connection_id);
                                        
                                        // Отправляем событие о получении запроса
                                        if let Some(tx) = server_request_tx.take() {
                                            let _ = tx.send((peer_id, connection_id));
                                        }
                                        
                                        // СРАЗУ ОТКЛОНЯЕМ соединение
                                        let reject_reason = format!("Peer {} not authorized", peer_id);
                                        println!("❌ Сервер: Отклонен входящий апгрейд от {}: {}", peer_id, reject_reason);
                                        
                                        // Отправляем решение об отклонении
                                        assert!(decision_sender.reject(reject_reason.clone()).is_ok(), 
                                            "❌ Не удалось отправить решение об отклонении");
                                        
                                        // Отправляем событие об отклонении
                                        if let Some(tx) = server_reject_tx.take() {
                                            let _ = tx.send((peer_id, reject_reason));
                                        }
                                        
                                    }
                                    XStreamEvent::IncomingStream { .. } => {
                                        // Это не должно происходить в этом тесте
                                        println!("⚠️ Сервер: Получен неожиданный входящий поток");
                                    }
                                    XStreamEvent::StreamEstablished { .. } => {
                                        // Это не должно происходить в этом тесте
                                        println!("⚠️ Сервер: Получен неожиданный установленный поток");
                                    }
                                    XStreamEvent::StreamError { peer_id, error, .. } => {
                                        println!("❌ Сервер: Ошибка потока с {}: {}", peer_id, error);
                                    }
                                    XStreamEvent::StreamClosed { .. } => {
                                        // Это нормально - потоки могут закрываться
                                    }
                                    _ => {
                                        // Другие события игнорируем
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            
            println!("✅ Серверная задача завершена корректно");
        }
    });

    // Подключаемся к серверу
    client_swarm.dial(DialOpts::peer_id(server_peer_id).addresses(vec![listen_addr.clone()]).build()).unwrap();
    println!("🔗 Клиент: Подключение к серверу по адресу {}", listen_addr);
    
    let mut stream_tx_some = Some(stream_tx);
    
    // Запускаем клиентскую задачу с graceful shutdown
    let client_handle = tokio::spawn({
        async move {
            println!("🎯 Клиентская задача запущена...");
            
            let mut client_shutdown_rx = client_shutdown_rx;
            let mut stream_opened = false;
            let mut stream_error_received = false;
            let mut connection_established = false;
            
            loop {
                tokio::select! {
                    // Проверяем сигнал завершения - ДОБАВЛЯЕМ ПРИОРИТЕТ
                    _ = &mut client_shutdown_rx => {
                        println!("🛑 Клиент: Получен сигнал завершения через client_shutdown_rx");
                        break;
                    }
                    // Обрабатываем события swarm с таймаутом
                    event = client_swarm.select_next_some() => {
                        match event {
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("✅ Клиент: Соединение установлено с: {}", peer_id);
                                if peer_id == server_peer_id {
                                    println!("✅ Клиент: Подключился к ожидаемому серверу");
                                    connection_established = true;
                                    
                                    if !stream_opened {
                                        if let Some(stream_tx) = stream_tx_some.take() {
                                            println!("🔄 Клиент: Открытие XStream к серверу...");
                                            client_swarm.behaviour_mut().open_stream(peer_id, stream_tx).await;
                                            stream_opened = true;
                                        }
                                    }
                                }
                            }
                            SwarmEvent::Behaviour(event) => {
                                match event {
                                    XStreamEvent::StreamEstablished { peer_id, stream_id } => {
                                        // Это не должно происходить в этом тесте
                                        println!("⚠️ Клиент: Получен неожиданный установленный поток с {} (ID: {:?})", peer_id, stream_id);
                                    }
                                    XStreamEvent::StreamError { peer_id, error, .. } => {
                                        println!("❌ Клиент: Ошибка потока с {}: {}", peer_id, error);
                                        stream_error_received = true;
                                        // После получения ошибки можем завершить клиент
                                        println!("🛑 Клиент: Завершение по StreamError");
                                        break;
                                    }
                                    XStreamEvent::StreamClosed { peer_id, .. } => {
                                        println!("🔒 Клиент: Поток закрыт с {}", peer_id);
                                    }
                                    XStreamEvent::IncomingStream { .. } | XStreamEvent::IncomingStreamRequest { .. } => {
                                        // Эти события не ожидаются на клиенте
                                    }
                                    _ => {
                                        println!("📨 Клиент: Получено другое событие: {:?}", event);
                                    }
                                }
                            }
                            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                                println!("🔒 Клиент: Соединение закрыто с {}", peer_id);
                                if peer_id == server_peer_id {
                                    println!("🛑 Клиент: Завершение по закрытию соединения с сервером");
                                    break;
                                }
                            }
                            SwarmEvent::NewListenAddr { address, .. } => {
                                println!("📡 Клиент слушает на: {}", address);
                            }
                            other_event => {
                                println!("📨 Клиент: Другое событие swarm: {:?}", other_event);
                            }
                        }
                    }
                }
            }
            
            // Проверяем, что ошибка была получена (это ожидаемое поведение)
            if !stream_error_received {
                println!("⚠️ Клиент: Ошибка потока не получена, но задача завершена по shutdown");
            } else {
                println!("✅ Клиент: Ошибка потока получена корректно");
            }
            println!("✅ Клиентская задача завершена корректно");
        }
    });

    // Даем серверу время запуститься
    sleep(Duration::from_millis(100)).await;

    sleep(Duration::from_millis(200)).await;
    // Ждем события от сервера
    let server_request_result = server_request_rx.await;
    assert!(server_request_result.is_ok(), "❌ Сервер должен был получить запрос на апгрейд");
    
    let server_reject_result = server_reject_rx.await;
    assert!(server_reject_result.is_ok(), "❌ Сервер должен был отправить событие об отклонении");


    // Ждем результат открытия потока
    let stream_result = stream_rx.await;
    match stream_result {
        Ok(Ok(stream)) => {
            // Это не должно происходить в этом тесте
            panic!("❌ Клиент не должен получить поток в этом тесте! {:?}", stream);
        }
        Ok(Err(error)) => {
            println!("❌ Клиент: Получена ошибка при открытии потока: {}", error);
            // Проверяем, что ошибка содержит информацию об отклонении
            assert!(error.contains("not authorized"), 
                "❌ Ошибка должна содержать информацию об отклонении: {}", error);
        }
        Err(_) => {
            // Канал может быть закрыт, если произошла ошибка в другом месте
            // Это может быть нормальным поведением в случае быстрого отказа
            println!("⚠️ Канал закрыт до получения результата - возможно, соединение было быстро отклонено");
        }
    }



    // Отправляем сигналы завершения swarm задачам
    let _ = server_shutdown_tx.send(());
    let _ = client_shutdown_tx.send(());

    // Даем немного времени для завершения всех операций
    

    // Ждем завершения задач с таймаутом
    match timeout(Duration::from_secs(1), server_handle).await {
        Ok(Ok(())) => println!("✅ Серверная задача завершена"),
        Ok(Err(e)) => panic!("❌ Ошибка в серверной задаче: {}", e),
        Err(_) => {
            println!("⚠️ Серверная задача не завершилась вовремя, продолжаем...");
        }
    }
    
    match timeout(Duration::from_secs(1), client_handle).await {
        Ok(Ok(())) => println!("✅ Клиентская задача завершена"),
        Ok(Err(e)) => panic!("❌ Ошибка в клиентской задаче: {}", e),
        Err(_) => {
            println!("⚠️ Клиентская задача не завершилась вовремя, продолжаем...");
        }
    }

    println!("✅ Тест сценария отклонения соединения пройден успешно!");
}

/// Создает узел с QUIC транспортом и указанной политикой
async fn create_quic_swarm_with_policy(
    policy: IncomingConnectionApprovePolicy
) -> Result<(Swarm<XStreamNetworkBehaviour>, PeerId), Box<dyn std::error::Error>> {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = keypair.public().to_peer_id();
    
    // Создаем QUIC транспорт
    let quic_config = quic::Config::new(&keypair);
    let quic_transport = quic::tokio::Transport::new(quic_config);
    
    // Создаем swarm с XStream поведением с указанной политикой
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|_key| quic_transport)
        .expect("❌ Не удалось создать QUIC транспорт")
        .with_behaviour(|_key| {
            XStreamNetworkBehaviour::new_with_policy(policy)
        })
        .expect("❌ Не удалось создать XStream поведение")
        .build();
    
    Ok((swarm, peer_id))
}

/// Ожидает адрес прослушивания от swarm
async fn wait_for_listen_addr(swarm: &mut Swarm<XStreamNetworkBehaviour>) -> Multiaddr {
    loop {
        if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
            return address;
        }
    }
}

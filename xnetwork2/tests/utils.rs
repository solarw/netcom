//! Утилиты для упрощения написания тестов XNetwork2

use std::time::Duration;
use tokio::time::timeout;
use xnetwork2::{
    main_behaviour::XNetworkCommands,
    node::Node,
    node_events::NodeEvent,
    swarm_commands::SwarmLevelCommand,
};
use libp2p::Multiaddr;

/// Утилита для ожидания конкретного события с таймаутом
pub async fn wait_for_event<F>(
    events: &mut tokio::sync::broadcast::Receiver<NodeEvent>,
    predicate: F,
    timeout_duration: Duration,
) -> Result<NodeEvent, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(&NodeEvent) -> bool,
{
    timeout(timeout_duration, async {
        loop {
            match events.recv().await {
                Ok(event) => {
                    if predicate(&event) {
                        return Ok(event);
                    }
                }
                Err(e) => {
                    return Err(format!(
                        "❌ Ошибка получения события: {} - система событий не работает",
                        e
                    )
                    .into());
                }
            }
        }
    })
    .await?
}

/// Утилита для ожидания двух событий в неизвестном порядке
pub async fn wait_for_two_events<F1, F2>(
    events1: &mut tokio::sync::broadcast::Receiver<NodeEvent>,
    events2: &mut tokio::sync::broadcast::Receiver<NodeEvent>,
    predicate1: F1,
    predicate2: F2,
    timeout_duration: Duration,
) -> Result<(NodeEvent, NodeEvent), Box<dyn std::error::Error + Send + Sync>>
where
    F1: Fn(&NodeEvent) -> bool,
    F2: Fn(&NodeEvent) -> bool,
{
    timeout(timeout_duration, async {
        let mut event1_opt = None;
        let mut event2_opt = None;

        while event1_opt.is_none() || event2_opt.is_none() {
            tokio::select! {
                Ok(event) = events1.recv() => {
                    if predicate1(&event) && event1_opt.is_none() {
                        event1_opt = Some(event);
                    }
                }
                Ok(event) = events2.recv() => {
                    if predicate2(&event) && event2_opt.is_none() {
                        event2_opt = Some(event);
                    }
                }
            }
        }

        Ok((event1_opt.unwrap(), event2_opt.unwrap()))
    })
    .await?
}

/// Запускает задачу ожидания и автоматического подтверждения PoR запроса
/// Возвращает JoinHandle для последующего ожидания
#[allow(dead_code)]
pub fn spawn_por_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    let mut events = node.subscribe();
    let commander = node.commander.clone();

    tokio::spawn(async move {
        println!("⏳ Ожидаем VerifyPorRequest для пира {} (таймаут {} секунд)...", expected_peer_id, timeout_duration.as_secs());
        
        // Ждем VerifyPorRequest для ожидаемого пира
        let por_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::VerifyPorRequest { peer_id, .. } if *peer_id == expected_peer_id),
            timeout_duration,
        ).await?;

        // Немедленно одобряем аутентификацию
        if let NodeEvent::VerifyPorRequest { peer_id, .. } = por_event {
            println!("✅ Получен VerifyPorRequest для пира {}, подтверждаем аутентификацию...", peer_id);
            commander.submit_por_verification(peer_id, true).await
                .expect(&format!("❌ Не удалось подтвердить аутентификацию для пира {} - критическая ошибка", peer_id));
            println!("✅ Аутентификация для пира {} успешно подтверждена", peer_id);
        }

        Ok(())
    })
}

/// Настраивает ноду для прослушивания и возвращает адрес
/// Автоматически отправляет ListenOn и ожидает NewListenAddr
#[allow(dead_code)]
pub async fn setup_listening_node(node: &mut Node) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
    println!("🎯 Настраиваем ноду для прослушивания...");

    // Сначала создаем подписку на события
    let mut events = node.subscribe();
    
    // Запускаем задачу ожидания события NewListenAddr ДО выполнения команды
    let listen_addr_future = async {
        println!("⏳ Ожидаем событие NewListenAddr (таймаут 5 секунд)...");
        let listen_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::NewListenAddr { .. }),
            Duration::from_secs(5)
        ).await.expect("❌ Таймаут ожидания события NewListenAddr - событие не пришло за 5 секунд");

        let listen_addr = match listen_event {
            NodeEvent::NewListenAddr { address, listener_id: _ } => address,
            _ => panic!("❌ Получено неожиданное событие: {:?}", listen_event),
        };

        println!("✅ Нода слушает на адресе: {}", listen_addr);
        listen_addr
    };

    // Выполнить ListenOn для ноды
    let (listen_response, listen_receiver) = tokio::sync::oneshot::channel();
    node.commander
        .send(XNetworkCommands::SwarmLevel(
            SwarmLevelCommand::ListenOn { 
                addr: "/ip4/127.0.0.1/udp/0/quic-v1".parse().expect("❌ Не удалось распарсить QUIC адрес"),
                response: listen_response 
            }
        ))
        .await
        .expect("❌ Не удалось отправить команду ListenOn - критическая ошибка");

    let listen_result = timeout(Duration::from_secs(5), listen_receiver)
        .await
        .expect("❌ Таймаут команды ListenOn")
        .expect("❌ Не удалось получить ответ ListenOn");

    assert!(listen_result.is_ok(), "❌ Нода должна слушать на QUIC адресе");
    println!("✅ Команда ListenOn выполнена успешно");

    // Ждем завершения задачи ожидания события
    let listen_addr = listen_addr_future.await;
    Ok(listen_addr)
}



#[allow(dead_code)]
pub async fn setup_listening_node_with_addr(node: &mut Node, addr: String) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
    println!("🎯 Настраиваем ноду для прослушивания...");

    // Сначала создаем подписку на события
    let mut events = node.subscribe();
    
    // Запускаем задачу ожидания события NewListenAddr ДО выполнения команды
    let listen_addr_future = async {
        println!("⏳ Ожидаем событие NewListenAddr (таймаут 5 секунд)...");
        let listen_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::NewListenAddr { .. }),
            Duration::from_secs(5)
        ).await.expect("❌ Таймаут ожидания события NewListenAddr - событие не пришло за 5 секунд");

        let listen_addr = match listen_event {
            NodeEvent::NewListenAddr { address, listener_id: _ } => address,
            _ => {println!("❌ Получено неожиданное событие: {:?}", listen_event); panic!("❌ Получено неожиданное событие: {:?}", listen_event)},
        };

        println!("✅ Нода слушает на адресе: {}", listen_addr);
        listen_addr
    };

    // Выполнить ListenOn для ноды
    let (listen_response, listen_receiver) = tokio::sync::oneshot::channel();
    node.commander
        .send(XNetworkCommands::SwarmLevel(
            SwarmLevelCommand::ListenOn { 
                addr: addr.parse().expect("❌ Не удалось распарсить QUIC адрес"),
                response: listen_response 
            }
        ))
        .await
        .expect("❌ Не удалось отправить команду ListenOn - критическая ошибка");

    let listen_result = timeout(Duration::from_secs(5), listen_receiver)
        .await
        .expect("❌ Таймаут команды ListenOn")
        .expect("❌ Не удалось получить ответ ListenOn");

    assert!(listen_result.is_ok(), "❌ Нода должна слушать на QUIC адресе");
    println!("✅ Команда ListenOn выполнена успешно");

    // Ждем завершения задачи ожидания события
    let listen_addr = listen_addr_future.await;
    Ok(listen_addr)
}

/// Запускает задачу ожидания события ConnectionEstablished
#[allow(dead_code)]
pub fn spawn_connection_established_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<libp2p::swarm::ConnectionId, Box<dyn std::error::Error + Send + Sync>>> {
    let mut events = node.subscribe();

    tokio::spawn(async move {
        println!("⏳ Ожидаем ConnectionEstablished для пира {} (таймаут {} секунд)...", expected_peer_id, timeout_duration.as_secs());
        
        let connection_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::ConnectionEstablished { peer_id, .. } if *peer_id == expected_peer_id),
            timeout_duration,
        ).await?;

        match connection_event {
            NodeEvent::ConnectionEstablished { connection_id, .. } => {
                println!("✅ Получен ConnectionEstablished для пира {}, connection_id: {:?}", expected_peer_id, connection_id);
                Ok(connection_id)
            }
            _ => Err("❌ Не удалось получить connection_id - получено неожиданное событие".into()),
        }
    })
}

/// Выполняет Dial и ожидает установки соединения
#[allow(dead_code)]
pub async fn dial_and_wait_connection(
    node: &mut Node,
    peer_id: libp2p::PeerId,
    addr: Multiaddr,
    timeout_duration: Duration,
) -> Result<libp2p::swarm::ConnectionId, Box<dyn std::error::Error + Send + Sync>> {
    println!("🔗 Выполняем Dial к пиру {}...", peer_id);

    // Запускаем задачу ожидания соединения ДО Dial
    let connection_task = spawn_connection_established_task(node, peer_id, timeout_duration);

    // Выполняем Dial
    let (dial_response, dial_receiver) = tokio::sync::oneshot::channel();
    node.commander
        .send(XNetworkCommands::SwarmLevel(
            SwarmLevelCommand::Dial {
                peer_id,
                addr: addr.clone(),
                response: dial_response,
            }
        ))
        .await
        .expect("❌ Не удалось отправить команду Dial - критическая ошибка");

    let dial_result = timeout(timeout_duration, dial_receiver)
        .await
        .expect("❌ Таймаут команды Dial")
        .expect("❌ Не удалось получить ответ Dial");

    assert!(dial_result.is_ok(), "❌ Должен подключиться к пиру {}", peer_id);
    println!("✅ Команда Dial выполнена успешно");

    // Ожидаем установки соединения и получаем connection_id
    let connection_id = connection_task.await
        .expect("❌ Задача ожидания соединения завершилась с ошибкой (join)")
        .expect("❌ Задача ожидания соединения завершилась с ошибкой (task)");

    println!("✅ Соединение с пиром {} успешно установлено, connection_id: {:?}", peer_id, connection_id);
    Ok(connection_id)
}

/// Полный цикл установки соединения с автоматической аутентификацией
#[allow(dead_code)]
pub async fn setup_connection_with_auth(
    node_a: &mut Node,
    node_b: &mut Node,
    addr_b: Multiaddr,
    timeout_duration: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Настраиваем соединение с аутентификацией между {} и {}...", node_a.peer_id(), node_b.peer_id());

    // Запускаем задачи ожидания PoR ДО Dial
    let por_task_a_to_b = spawn_por_task(node_a, *node_b.peer_id(), timeout_duration);
    let por_task_b_to_a = spawn_por_task(node_b, *node_a.peer_id(), timeout_duration);

    // Запускаем задачу ожидания соединения на node_b ДО Dial
    let connection_task_b = spawn_connection_established_task(node_b, *node_a.peer_id(), timeout_duration);

    // Выполняем Dial и ожидаем соединения на node_a
    let connection_id_a = dial_and_wait_connection(node_a, *node_b.peer_id(), addr_b, timeout_duration).await?;

    // Ожидаем соединения на node_b
    let connection_id_b = connection_task_b.await
        .expect("❌ Задача ожидания соединения на node_b завершилась с ошибкой (join)")
        .expect("❌ Задача ожидания соединения на node_b завершилась с ошибкой (task)");

    // После установки соединения явно запускаем аутентификацию на ОБЕИХ сторонах
    // В ручном режиме аутентификация должна запускаться явно на обеих сторонах
    println!("🔄 Запускаем ручную аутентификацию на node_a для connection_id: {:?}", connection_id_a);
    node_a.commander.start_auth_for_connection(connection_id_a).await
        .expect("❌ Не удалось запустить аутентификацию на node_a - критическая ошибка");

    println!("🔄 Запускаем ручную аутентификацию на node_b для connection_id: {:?}", connection_id_b);
    node_b.commander.start_auth_for_connection(connection_id_b).await
        .expect("❌ Не удалось запустить аутентификацию на node_b - критическая ошибка");

    // Ждем завершения аутентификации
    println!("⏳ Ждем завершения аутентификации...");
    por_task_a_to_b.await
        .expect("❌ Задача PoR для A → B завершилась с ошибкой (join)")
        .expect("❌ Задача PoR для A → B завершилась с ошибкой (task)");
    por_task_b_to_a.await
        .expect("❌ Задача PoR для B → A завершилась с ошибкой (join)")
        .expect("❌ Задача PoR для B → A завершилась с ошибкой (task)");

    println!("✅ Аутентификация успешно завершена");
    Ok(())
}

/// Запускает задачу ожидания VerifyPorRequest в ручном режиме (без автоматического подтверждения)
/// Возвращает JoinHandle и Receiver для получения события
#[allow(dead_code)]
pub fn spawn_manual_por_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> (
    tokio::task::JoinHandle<Result<NodeEvent, Box<dyn std::error::Error + Send + Sync>>>,
    tokio::sync::broadcast::Receiver<NodeEvent>,
) {
    let mut events = node.subscribe();
    let events_clone = events.resubscribe();

    let handle = tokio::spawn(async move {
        println!("⏳ Ожидаем VerifyPorRequest для пира {} в ручном режиме (таймаут {} секунд)...", 
                expected_peer_id, timeout_duration.as_secs());
        
        // Ждем VerifyPorRequest для ожидаемого пира
        let por_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::VerifyPorRequest { peer_id, .. } if *peer_id == expected_peer_id),
            timeout_duration,
        ).await?;

        println!("✅ Получен VerifyPorRequest для пира {} в ручном режиме", expected_peer_id);
        Ok(por_event)
    });

    (handle, events_clone)
}

/// Ожидает события VerifyPorRequest на обеих нодах в ручном режиме
#[allow(dead_code)]
pub async fn wait_for_manual_por_requests(
    node1: &mut Node,
    node2: &mut Node,
    timeout_duration: Duration,
) -> Result<(NodeEvent, NodeEvent), Box<dyn std::error::Error + Send + Sync>> {
    println!("🔐 Ожидаем VerifyPorRequest на обеих нодах в ручном режиме...");

    // Запускаем задачи ожидания на обеих нодах
    let (task1, mut events1) = spawn_manual_por_task(node1, *node2.peer_id(), timeout_duration);
    let (task2, mut events2) = spawn_manual_por_task(node2, *node1.peer_id(), timeout_duration);

    // Используем wait_for_two_events для ожидания обоих событий
    let (event1, event2) = wait_for_two_events(
        &mut events1,
        &mut events2,
        |e| matches!(e, NodeEvent::VerifyPorRequest { .. }),
        |e| matches!(e, NodeEvent::VerifyPorRequest { .. }),
        timeout_duration,
    ).await?;

    // Ждем завершения задач
    task1.await
        .expect("❌ Задача ожидания PoR для ноды1 завершилась с ошибкой (join)")
        .expect("❌ Задача ожидания PoR для ноды1 завершилась с ошибкой (task)");
    task2.await
        .expect("❌ Задача ожидания PoR для ноды2 завершилась с ошибкой (join)")
        .expect("❌ Задача ожидания PoR для ноды2 завершилась с ошибкой (task)");

    println!("✅ Оба VerifyPorRequest получены в ручном режиме");
    Ok((event1, event2))
}

/// Утилита для проверки отсутствия автоматической аутентификации
#[allow(dead_code)]
pub async fn assert_no_auth_events(
    node1: &mut Node,
    node2: &mut Node,
    duration: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🔍 Проверяем отсутствие автоматической аутентификации в течение {} секунд...", duration.as_secs());
    
    let mut events1 = node1.subscribe();
    let mut events2 = node2.subscribe();
    
    let result = timeout(duration, async {
        loop {
            tokio::select! {
                Ok(event) = events1.recv() => {
                    if matches!(event, 
                        NodeEvent::PeerMutualAuthSuccess { .. } |
                        NodeEvent::PeerOutboundAuthSuccess { .. } |
                        NodeEvent::PeerInboundAuthSuccess { .. }
                    ) {
                        return Err::<(), _>("❌ Нода1 получила событие успешной аутентификации в ручном режиме".into());
                    }
                }
                Ok(event) = events2.recv() => {
                    if matches!(event, 
                        NodeEvent::PeerMutualAuthSuccess { .. } |
                        NodeEvent::PeerOutboundAuthSuccess { .. } |
                        NodeEvent::PeerInboundAuthSuccess { .. }
                    ) {
                        return Err::<(), _>("❌ Нода2 получила событие успешной аутентификации в ручном режиме".into());
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Периодическая проверка завершения
                }
            }
        }
    }).await;

    match result {
        Ok(Err(e)) => Err(e),
        Ok(Ok(_)) => panic!("❌ Неожиданное завершение цикла проверки аутентификации"),
        Err(_) => {
            println!("✅ Автоматическая аутентификация не произошла в течение {} секунд", duration.as_secs());
            Ok(())
        }
    }
}

/// Получает connection_id для указанного пира из состояния сети
#[allow(dead_code)]
pub async fn get_connection_id(
    node: &mut Node,
    peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> Result<libp2p::swarm::ConnectionId, Box<dyn std::error::Error + Send + Sync>> {
    println!("🔍 Получаем connection_id для пира {}...", peer_id);
    
    let mut events = node.subscribe();
    let connection_event = wait_for_event(
        &mut events,
        |e| matches!(e, NodeEvent::ConnectionEstablished { peer_id: event_peer_id, .. } if *event_peer_id == peer_id),
        timeout_duration,
    ).await?;

    match connection_event {
        NodeEvent::ConnectionEstablished { connection_id, .. } => {
            println!("✅ Получен connection_id: {:?} для пира {}", connection_id, peer_id);
            Ok(connection_id)
        }
        _ => Err("❌ Не удалось получить connection_id - получено неожиданное событие".into()),
    }
}

/// Настраивает ноду для прослушивания и автоматически добавляет адрес в Kademlia как внешний
/// Объединяет setup_listening_node и add_external_address в одну операцию
#[allow(dead_code)]
pub async fn setup_listening_node_with_kad(
    node: &mut Node,
) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
    println!("🎯 Настраиваем ноду для прослушивания с автоматической регистрацией в Kademlia...");

    // Сначала настраиваем прослушивание
    let listen_addr = setup_listening_node(node).await?;
    println!("📡 Нода слушает на адресе: {}", listen_addr);

    // Затем добавляем адрес как внешний для Kademlia
    println!("🌐 Добавляем адрес как внешний для Kademlia...");
    node.commander.add_external_address(listen_addr.clone()).await?;
    println!("✅ Адрес {} успешно добавлен как внешний для Kademlia", listen_addr);

    Ok(listen_addr)
}

/// Запускает задачу ожидания ConnectionEstablished для получения connection_id
/// Должна запускаться ДО dial_and_wait_connection
#[allow(dead_code)]
pub fn spawn_connection_id_listener_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<libp2p::swarm::ConnectionId, Box<dyn std::error::Error + Send + Sync>>> {
    let mut events = node.subscribe();

    tokio::spawn(async move {
        println!("⏳ Ожидаем ConnectionEstablished для пира {} (таймаут {} секунд)...", 
                expected_peer_id, timeout_duration.as_secs());
        
        let connection_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::ConnectionEstablished { peer_id, .. } if *peer_id == expected_peer_id),
            timeout_duration,
        ).await?;

        match connection_event {
            NodeEvent::ConnectionEstablished { connection_id, .. } => {
                println!("✅ Получен ConnectionEstablished для пира {}, connection_id: {:?}", 
                        expected_peer_id, connection_id);
                Ok(connection_id)
            }
            _ => Err("❌ Не удалось получить connection_id - получено неожиданное событие".into()),
        }
    })
}

/// Создает асинхронную задачу, которая ждет завершения аутентификации для указанного пира
/// Ожидает любое из трех событий успешной аутентификации: MutualAuthSuccess, OutboundAuthSuccess, InboundAuthSuccess
#[allow(dead_code)]
pub fn spawn_auth_completion_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    let mut events = node.subscribe();

    tokio::spawn(async move {
        println!("⏳ Ожидаем завершение аутентификации для пира {} (таймаут {} секунд)...", 
                expected_peer_id, timeout_duration.as_secs());
        
        let _auth_event = wait_for_event(
            &mut events,
            |e| {
                matches!(e, 
                    NodeEvent::PeerMutualAuthSuccess { peer_id, .. } if *peer_id == expected_peer_id
                ) || matches!(e, 
                    NodeEvent::PeerOutboundAuthSuccess { peer_id, .. } if *peer_id == expected_peer_id
                ) || matches!(e, 
                    NodeEvent::PeerInboundAuthSuccess { peer_id, .. } if *peer_id == expected_peer_id
                )
            },
            timeout_duration,
        ).await?;

        println!("✅ Аутентификация завершена для пира {}", expected_peer_id);
        Ok(())
    })
}

/// Создает асинхронную задачу, которая ждет VerifyPorRequest и сразу подтверждает его
#[allow(dead_code)]
pub fn spawn_auto_respond_por_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    let mut events = node.subscribe();
    let commander = node.commander.clone();

    tokio::spawn(async move {
        println!("⏳ Ожидаем VerifyPorRequest от пира {} (таймаут {} секунд)...", 
                expected_peer_id, timeout_duration.as_secs());
        
        let por_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::VerifyPorRequest { peer_id, .. } if *peer_id == expected_peer_id),
            timeout_duration,
        ).await?;

        // Немедленно подтверждаем аутентификацию
        if let NodeEvent::VerifyPorRequest { peer_id, .. } = por_event {
            println!("✅ Получен VerifyPorRequest от пира {}, подтверждаем аутентификацию...", peer_id);
            commander.submit_por_verification(peer_id, true).await
                .expect(&format!("❌ Не удалось подтвердить аутентификацию для пира {} - критическая ошибка", peer_id));
            println!("✅ Аутентификация для пира {} успешно подтверждена", peer_id);
        }

        Ok(())
    })
}

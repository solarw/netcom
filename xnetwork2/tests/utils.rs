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
            NodeEvent::NewListenAddr { address } => address,
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

/// Запускает задачу ожидания события ConnectionEstablished
pub fn spawn_connection_established_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<NodeEvent, Box<dyn std::error::Error + Send + Sync>>> {
    let mut events = node.subscribe();

    tokio::spawn(async move {
        println!("⏳ Ожидаем ConnectionEstablished для пира {} (таймаут {} секунд)...", expected_peer_id, timeout_duration.as_secs());
        
        let connection_event = wait_for_event(
            &mut events,
            |e| matches!(e, NodeEvent::ConnectionEstablished { peer_id, .. } if *peer_id == expected_peer_id),
            timeout_duration,
        ).await?;

        println!("✅ Получен ConnectionEstablished для пира {}", expected_peer_id);
        Ok(connection_event)
    })
}

/// Выполняет Dial и ожидает установки соединения
pub async fn dial_and_wait_connection(
    node: &mut Node,
    peer_id: libp2p::PeerId,
    addr: Multiaddr,
    timeout_duration: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    // Ожидаем установки соединения
    connection_task.await
        .expect("❌ Задача ожидания соединения завершилась с ошибкой (join)")
        .expect("❌ Задача ожидания соединения завершилась с ошибкой (task)");

    println!("✅ Соединение с пиром {} успешно установлено", peer_id);
    Ok(())
}

/// Полный цикл установки соединения с автоматической аутентификацией
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

    // Выполняем Dial и ожидаем соединения
    dial_and_wait_connection(node_a, *node_b.peer_id(), addr_b, timeout_duration).await?;

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

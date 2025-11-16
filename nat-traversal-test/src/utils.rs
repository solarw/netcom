//! Утилиты для программ relay и node
//! Скопировано из xnetwork2/tests/utils.rs

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
                addr: "/ip4/0.0.0.0/udp/0/quic-v1".parse().expect("❌ Не удалось распарсить QUIC адрес"),
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

/// Настраивает ноду для прослушивания на указанном адресе
pub async fn setup_listening_node_with_addr(node: &mut Node, addr: String) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
    println!("🎯 Настраиваем ноду для прослушивания на адресе {}...", addr);

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
            _ => {println!("❌ Получено неожиданное событие: {:?}", listen_event); panic!("oops");},
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

/// Запускает задачу ожидания ConnectionEstablished
fn spawn_connection_established_task(
    node: &mut Node,
    expected_peer_id: libp2p::PeerId,
    timeout_duration: Duration,
) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
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
                Ok(())
            }
            _ => Err("❌ Не удалось получить connection_id - получено неожиданное событие".into()),
        }
    })
}

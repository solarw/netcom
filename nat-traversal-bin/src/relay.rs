use std::env;
use base64::prelude::*;
use xnetwork2::node_builder::NodeBuilder;

mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запускаем relay сервер...");

    // Загружаем ключ из переменной окружения
    let key_bytes = if let Ok(key_env) = env::var("NODE_KEY") {
        println!("🔑 Загружаем ключ из переменной окружения...");
        BASE64_STANDARD.decode(&key_env)?
    } else {
        println!("🔑 Генерируем новый ключ...");
        return Err("❌ NODE_KEY не установлена - требуется ключ для relay".into());
    };

    // Создаем relay сервер
    println!("🛠️ Создаем relay сервер...");
    let mut relay = NodeBuilder::new()
        .with_fixed_key(key_bytes)
        .with_relay_server()
        .with_kademlia()
        .build()
        .await?;

    println!("✅ Relay сервер создан, peer_id: {}", relay.peer_id());

    // Запускаем relay
    println!("▶️ Запускаем relay сервер...");
    relay.start().await?;

    // Настраиваем прослушивание на фиксированном порту
    println!("🎯 Настраиваем прослушивание на порту 15003...");
    let relay_addr = setup_listening_node_with_addr(&mut relay, "/ip4/0.0.0.0/udp/15003/quic-v1".to_string()).await?;
    println!("📡 Relay сервер слушает на: {}", relay_addr);
    
    // Ждем немного чтобы все интерфейсы были готовы
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Включаем Kademlia DHT
    println!("🌐 Включаем Kademlia DHT...");
    relay.commander.enable_kad().await?;
    println!("✅ Kademlia DHT включена");

    println!("✅ Relay сервер готов к работе!");
    println!("💡 Peer ID: {}", relay.peer_id());
    println!("📡 Адрес: {}", relay_addr);

    // Бесконечный цикл для поддержания работы сервера
    println!("⏳ Ожидаем сигнал завершения...");
    tokio::signal::ctrl_c().await?;
    println!("🛑 Получен сигнал завершения...");

    // Корректное завершение
    println!("🧹 Завершаем работу relay сервера...");
    relay.force_shutdown().await?;
    println!("✅ Relay сервер завершен");

    Ok(())
}

/// Упрощенная версия setup_listening_node_with_addr для relay
async fn setup_listening_node_with_addr(node: &mut xnetwork2::node::Node, addr: String) -> Result<libp2p::Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
    use xnetwork2::{
        main_behaviour::XNetworkCommands,
        swarm_commands::SwarmLevelCommand,
    };
    use tokio::time::timeout;
    use std::time::Duration;

    println!("🎯 Настраиваем прослушивание на адресе {}...", addr);

    // Сначала создаем подписку на события
    let mut events = node.subscribe();
    
    // Запускаем задачу ожидания события NewListenAddr ДО выполнения команды
    let listen_addr_future = tokio::spawn(async move {
        println!("⏳ Ожидаем событие NewListenAddr (таймаут 5 секунд)...");
        
        // Ждем несколько адресов и выбираем не-localhost
        let mut addresses = Vec::new();
        let start = std::time::Instant::now();
        
        while start.elapsed() < Duration::from_secs(5) && addresses.len() < 5 {
            match utils::wait_for_event(
                &mut events,
                |e| matches!(e, xnetwork2::node_events::NodeEvent::NewListenAddr { .. }),
                Duration::from_secs(1)
            ).await {
                Ok(listen_event) => {
                    let listen_addr = match listen_event {
                        xnetwork2::node_events::NodeEvent::NewListenAddr { address, listener_id: _ } => address,
                        _ => panic!("❌ Получено неожиданное событие: {:?}", listen_event),
                    };
                    println!("📡 Получен адрес прослушивания: {}", listen_addr);
                    addresses.push(listen_addr);
                }
                Err(_) => {
                    // Таймаут - продолжаем ждать
                    continue;
                }
            }
        }
        
        // Выбираем первый не-localhost адрес
        let external_addr = addresses
            .into_iter()
            .find(|addr| !addr.to_string().contains("127.0.0.1"))
            .unwrap_or_else(|| {
                println!("⚠️ Не найден внешний адрес, используем первый доступный");
                addresses.first().expect("❌ Нет адресов прослушивания").clone()
            });
        
        println!("✅ Relay слушает на внешнем адресе: {}", external_addr);
        external_addr
    });

    // Выполнить ListenOn для relay
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

    assert!(listen_result.is_ok(), "❌ Relay должен слушать на QUIC адресе");
    println!("✅ Команда ListenOn выполнена успешно");

    // Ждем завершения задачи ожидания события
    let listen_addr = listen_addr_future.await
        .expect("❌ Задача ожидания адреса завершилась с ошибкой");
    Ok(listen_addr)
}

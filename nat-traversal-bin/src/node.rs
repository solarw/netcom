use base64::prelude::*;
use clap::Parser;
use libp2p::Multiaddr;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::node_builder::NodeBuilder;
use xnetwork2::xroutes::types::KadMode;
mod utils;

#[derive(Parser, Debug)]
#[command(version, about = "Node для тестирования NAT traversal")]
struct Args {
    /// Адрес relay сервера (например: 172.20.0.10:15003)
    #[arg(long)]
    relay_address: String,

    /// Peer ID relay сервера (обязательный для получения relay адреса)
    #[arg(long)]
    relay_peer_id: String,

    /// Peer ID узла для подключения (опционально)
    #[arg(long)]
    target_peer: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    println!("🚀 Запускаем node...");

    // Загружаем ключ из переменной окружения
    let key_bytes = if let Ok(key_env) = env::var("NODE_KEY") {
        println!("🔑 Загружаем ключ из переменной окружения...");
        BASE64_STANDARD.decode(&key_env)?
    } else {
        println!("🔑 Генерируем новый ключ...");
        return Err("❌ NODE_KEY не установлена - требуется ключ для node".into());
    };

    // Создаем node
    println!("🛠️ Создаем node...");
    let mut node = NodeBuilder::new()
        .with_fixed_key(key_bytes)
        .with_kademlia()
        .with_autonat_client() // Включаем AutoNAT клиент для определения типа NAT
        .with_dcutr()
        .build()
        .await?;

    println!("✅ Node создан, peer_id: {}", node.peer_id());

    // Запускаем node
    println!("▶️ Запускаем node...");
    node.start().await?;

    // ВКЛЮЧАЕМ KADEMLIA ДО ПРОСЛУШИВАНИЯ
    println!("🌐 Включаем Kademlia DHT...");
    node.commander.enable_kad().await?;
    node.commander.set_kad_mode(KadMode::Server).await?;
    println!("✅ Kademlia DHT включена");
    println!("KAD MODE {:?}", node.commander.get_kad_mode().await);
    // Настраиваем прослушивание на случайном порту
    println!("🎯 Настраиваем прослушивание...");
    let node_addr = utils::setup_listening_node(&mut node).await?;
    println!("📡 Node слушает на: {}", node_addr);

    // Добавляем адрес прослушивания как внешний адрес
    //  нельзя добавлять!!!!
    //println!("🌐 Добавляем адрес прослушивания как внешний адрес...");
    //node.commander.add_external_address(node_addr.clone()).await?;
    //println!("✅ Внешний адрес добавлен: {}", node_addr);

    // Подключаемся к relay с повторными попытками
    println!("🔗 Подключаемся к relay серверу {}...", args.relay_address);
    connect_to_relay_with_retries(&mut node, &args.relay_address).await?;
    println!("✅ Подключение к relay установлено");

    sleep(Duration::from_millis(5000)).await;
    // Получаем relay адрес
    println!("🌐 Получаем relay адрес...");
    println!("KAD MODE {:?}", node.commander.get_kad_mode().await);
    sleep(Duration::from_millis(500)).await;
    let relay_addr = get_relay_address(&mut node, &args.relay_peer_id).await?;
    println!("✅ Relay адрес получен: {}", relay_addr);

    // Создаем адрес relay сервера для AutoNAT
    let (host, port) = if args.relay_address.contains(':') {
        let parts: Vec<&str> = args.relay_address.split(':').collect();
        (parts[0], parts[1])
    } else {
        (args.relay_address.as_str(), "15003")
    };

    let relay_server_addr: Multiaddr = if host.contains('.') {
        format!("/ip4/{}/udp/{}/quic-v1", host, port).parse()?
    } else {
        format!("/dns4/{}/udp/{}/quic-v1", host, port).parse()?
    };

    // Добавляем relay как AutoNAT сервер
    println!("🌐 Добавляем relay как AutoNAT сервер...");
    let relay_peer_id: libp2p::PeerId = args.relay_peer_id.parse()?;
    node.commander
        .add_autonat_server(relay_peer_id, Some(relay_server_addr.clone()))
        .await?;
    println!("✅ Relay добавлен как AutoNAT сервер");

    // Выводим все внешние адреса
    println!("🌐 Получаем все внешние адреса...");
    let external_addrs = node.commander.get_external_addresses().await?;
    println!("📊 Внешние адреса узла:");
    for (i, addr) in external_addrs.iter().enumerate() {
        println!("   {}. {}", i + 1, addr);
    }

    println!("00000000000000000000000000000000000000000 LONG SLEEP!");
    node.commander
        .bootstrap_to_peer(relay_peer_id, [relay_server_addr.clone()].to_vec())
        .await?;
    sleep(Duration::from_millis(5000)).await;
    println!("KAD MODE {:?}", node.commander.get_kad_mode().await);
    // Если указан target_peer, подключаемся к нему

    if let Some(target_peer_id_str) = &args.target_peer {
        println!("🎯 Ищем и подключаемся к пиру {}...", target_peer_id_str);
        let target_peer_id: libp2p::PeerId = target_peer_id_str.parse()?;

        // Ищем адреса пира в Kademlia с повторными попытками
        let target_addrs = find_peer_in_kademlia_with_retries(&mut node, target_peer_id).await?;
        println!("✅ Найдены адреса пира: {:?}", target_addrs);

        // Подключаемся через relay
        if let Some(relay_addr_for_target) = target_addrs
            .iter()
            .find(|addr| addr.to_string().contains("p2p-circuit"))
        {
            println!(
                "🔗 Подключаемся к пиру через relay: {}",
                relay_addr_for_target
            );
            utils::dial_and_wait_connection(
                &mut node,
                target_peer_id,
                relay_addr_for_target.clone(),
                Duration::from_secs(10),
            )
            .await?;
            println!("✅ Подключение к пиру установлено через relay!");
        } else {
            println!("❌ Не найден relay адрес для пира {}", target_peer_id);
        }
    }

    println!("✅ Node готов к работе!");
    println!("💡 Peer ID: {}", node.peer_id());
    println!("📡 Адрес: {}", node_addr);
    println!("🌐 Relay адрес: {}", relay_addr);

    // Подписываемся на события для обработки
    println!("📡 Подписываемся на события node...");
    let mut events = node.subscribe();

    // Если подключились к target_peer, завершаем работу после короткой паузы
    if args.target_peer.is_some() {
        println!("✅ NAT traversal успешен! Подключение к целевому пиру установлено.");
        println!("⏳ 111111111111111111111 Завершаем работу через 2 секунды...");
    }

    // Бесконечный цикл для поддержания работы и обработки событий
    println!("⏳ Ожидаем события и сигнал завершения...");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("🛑 Получен сигнал завершения...");
        }
        _ = async {
            loop {
                match events.recv().await {
                    Ok(event) => {
                        println!("📡 Получено событие: {:?}", event);
                        // Здесь можно обрабатывать события по мере необходимости
                    }
                    Err(e) => {
                        println!("❌ Ошибка получения события: {}", e);
                        break;
                    }
                }
            }
        } => {}
    }

    // Корректное завершение
    println!("🧹 Завершаем работу node...");
    node.force_shutdown().await?;
    println!("✅ Node завершен");

    Ok(())
}

/// Подключается к relay с повторными попытками
async fn connect_to_relay_with_retries(
    node: &mut xnetwork2::node::Node,
    relay_addr: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Разбираем адрес на хост и порт
    let (host, port) = if relay_addr.contains(':') {
        let parts: Vec<&str> = relay_addr.split(':').collect();
        (parts[0], parts[1])
    } else {
        (relay_addr, "15003")
    };

    // Создаем правильный multiaddr в зависимости от типа хоста
    let relay_multiaddr: Multiaddr = if host.contains('.') {
        // IPv4 адрес
        format!("/ip4/{}/udp/{}/quic-v1", host, port).parse()?
    } else {
        // DNS имя
        format!("/dns4/{}/udp/{}/quic-v1", host, port).parse()?
    };

    println!(
        "🔗 Пытаемся подключиться к relay по адресу: {}",
        relay_multiaddr
    );

    // Подписываемся на события для отслеживания соединений
    let mut events = node.subscribe();

    for attempt in 1..=10 {
        println!(
            "🔄 Попытка подключения к relay #{}/10 по адресу {}...",
            attempt, relay_multiaddr
        );

        // Пытаемся подключиться
        match node
            .commander
            .dial(
                libp2p::PeerId::random(), // Временный peer_id, будет заменен при реальном подключении
                relay_multiaddr.clone(),
            )
            .await
        {
            Ok(_) => {
                println!("✅ Команда dial отправлена, ожидаем установления соединения...");

                // Ждем события ConnectionEstablished в течение 5 секунд
                let timeout = Duration::from_secs(5);
                let start = std::time::Instant::now();

                while start.elapsed() < timeout {
                    match tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
                        Ok(Ok(event)) => {
                            println!("📡 Получено событие: {:?}", event);
                            // Проверяем, что соединение установлено
                            if let xnetwork2::node_events::NodeEvent::ConnectionEstablished {
                                peer_id,
                                ..
                            } = event
                            {
                                println!("✅ Соединение установлено с peer_id: {}", peer_id);
                                return Ok(());
                            }
                        }
                        Ok(Err(e)) => {
                            println!("❌ Ошибка получения события: {}", e);
                            break;
                        }
                        Err(_) => {
                            // Таймаут - продолжаем ждать
                            continue;
                        }
                    }
                }

                println!("⚠️ Соединение не установлено в течение таймаута, пробуем снова...");
            }
            Err(e) => {
                println!("❌ Ошибка отправки команды dial: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Err(format!(
        "❌ Не удалось подключиться к relay по адресу {} после 10 попыток",
        relay_multiaddr
    )
    .into())
}

/// Получает relay адрес через настройку прослушивания на специальном relay адресе
async fn get_relay_address(
    node: &mut xnetwork2::node::Node,
    relay_peer_id: &str,
) -> Result<Multiaddr, Box<dyn std::error::Error + Send + Sync>> {
    // Получаем адрес relay из аргументов командной строки
    let args = Args::parse();

    // Разбираем адрес на хост и порт
    let (host, port) = if args.relay_address.contains(':') {
        let parts: Vec<&str> = args.relay_address.split(':').collect();
        (parts[0], parts[1])
    } else {
        (args.relay_address.as_str(), "15003")
    };

    // Создаем правильный multiaddr в зависимости от типа хоста
    let relay_addr: Multiaddr = if host.contains('.') {
        // IPv4 адрес
        format!("/ip4/{}/udp/{}/quic-v1", host, port).parse()?
    } else {
        // DNS имя
        format!("/dns4/{}/udp/{}/quic-v1", host, port).parse()?
    };

    // Формируем relay адрес с правильным peer ID
    let relay_addr_str = format!(
        "{}/p2p/{}/p2p-circuit",
        relay_addr.to_string(),
        relay_peer_id
    );

    println!("🔗 Создаем relay адрес: {}", relay_addr_str);

    // Настраиваем прослушивание на relay адресе
    let node_relay_addr = utils::setup_listening_node_with_addr(node, relay_addr_str).await?;

    println!("✅ Relay адрес настроен: {}", node_relay_addr);
    Ok(node_relay_addr)
}

/// Ищет пира в Kademlia с повторными попытками
async fn find_peer_in_kademlia_with_retries(
    node: &mut xnetwork2::node::Node,
    peer_id: libp2p::PeerId,
) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
    for attempt in 1..=30 {
        println!("🔍 Поиск пира {} в Kademlia #{}/10...", peer_id, attempt);

        match node
            .commander
            .find_peer_addresses(peer_id, Duration::from_secs(5))
            .await
        {
            Ok(addrs) => {
                if !addrs.is_empty() {
                    println!("✅ Найдены адреса пира: {:?}", addrs);
                    return Ok(addrs);
                } else {
                    println!("⚠️ Адреса пира не найдены");
                }
            }
            Err(e) => {
                println!("❌ Ошибка поиска пира: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Err(format!(
        "❌ Не удалось найти пира {} в Kademlia после 10 попыток",
        peer_id
    )
    .into())
}

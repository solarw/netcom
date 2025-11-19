//! Тест подключения трех узлов без xauth

use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::{Node, NodeBuilder};

mod utils;
use utils::{dial_and_wait_connection, setup_listening_node};

use crate::utils::setup_listening_node_with_addr;

/// Тест подключения трех узлов без xauth
/// Создает три узла: server, node1, node2
/// Все узлы выполняют listen_on
/// node1 подключается к server
/// Как только подключение установлено - тест завершается
#[tokio::test]
async fn relay_connection_test() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let _ = tracing_subscriber::fmt::try_init();
    println!("🚀 Запускаем тест подключения трех узлов без xauth...");

    // ФАЗА 1: Создание трех узлов
    println!("🛠️ Фаза 1: Создание трех узлов...");

    // Создаем три узла с именами: server, node1, node2
    println!("🆕 Создаем server узел...");

    //let mut server = Node::new().await
    let mut server = NodeBuilder::new()
        .with_relay_server()
        .build()
        .await
        .expect("❌ Не удалось создать server узел - критическая ошибка");

    println!("🆕 Создаем node1 узел...");
    let mut node1 = NodeBuilder::new()
        .build()
        .await
        .expect("❌ Не удалось создать node1 узел - критическая ошибка");

    println!("🆕 Создаем node2 узел...");
    let mut node2 = NodeBuilder::new()
        .build()
        .await
        .expect("❌ Не удалось создать node2 узел - критическая ошибка");

    // Запуск всех узлов
    println!("🚀 Запускаем все узлы...");
    server
        .start()
        .await
        .expect("❌ Не удалось запустить server узел - критическая ошибка");
    node1
        .start()
        .await
        .expect("❌ Не удалось запустить node1 узел - критическая ошибка");
    node2
        .start()
        .await
        .expect("❌ Не удалось запустить node2 узел - критическая ошибка");

    // Небольшая задержка для запуска swarm loops
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("✅ Узлы созданы и запущены:");
    println!("   - Server: {:?}", server.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());

    // ФАЗА 2: Настройка всех узлов для прослушивания
    println!("🎯 Фаза 2: Настройка всех узлов для прослушивания...");

    // Настраиваем server узел для прослушивания
    println!("🎯 Настраиваем server узел для прослушивания...");
    let server_addr = setup_listening_node(&mut server).await?;
    println!("📡 Server узел слушает на: {}", server_addr);

    println!("🎯 Настраиваем внешний адрес server узел для прослушивания...");
    server.commander.add_external_address(server_addr.clone()).await?;
    println!("📡 Server узел слушает на внешнем: {}", server_addr);

    // Настраиваем node1 узел для прослушивания
    println!("🎯 Настраиваем node1 узел для прослушивания...");
    let node1_addr = setup_listening_node(&mut node1).await?;
    println!("📡 Node 1 слушает на: {}", node1_addr);

    // Настраиваем node2 узел для прослушивания
    println!("🎯 Настраиваем node2 узел для прослушивания...");
    let node2_addr = setup_listening_node(&mut node2).await?;
    println!("📡 Node 2 слушает на: {}", node2_addr);

    // Проверяем, что все адреса содержат QUIC
    assert!(
        server_addr.to_string().contains("/quic-v1"),
        "❌ Адрес server должен содержать QUIC протокол"
    );
    assert!(
        node1_addr.to_string().contains("/quic-v1"),
        "❌ Адрес node1 должен содержать QUIC протокол"
    );
    assert!(
        node2_addr.to_string().contains("/quic-v1"),
        "❌ Адрес node2 должен содержать QUIC протокол"
    );

    // ФАЗА 3: Подключение node1 к server
    println!("🔗 Фаза 3: Подключение node1 к server...");

    // Выполняем Dial от node1 к server и ожидаем установки соединения
    println!("🔗 Node1 подключается к server...");
    let _ = dial_and_wait_connection(
        &mut node1,
        *server.peer_id(),
        server_addr.clone(),
        Duration::from_secs(10),
    )
    .await?;

    let relay_addr_str = format!(
        "{}/p2p/{}/p2p-circuit",
        server_addr.to_string(),
        server.peer_id.to_string()
    );
    sleep(Duration::from_millis(100)).await;
    //panic!("Relay server address: {}", relay_addr_str);
    println!("Addr: {}", relay_addr_str);
    let node1_relay_addr = setup_listening_node_with_addr(&mut node1, relay_addr_str).await?;

    println!(
        "✅ Node1 успешно получил relay  listen address {:?}",
        node1_relay_addr
    );

    // Выполняем Dial от node2 к node1 через relay предоставленный server и ожидаем установки соединения
    println!("🔗 Node2 подключается к node1 via relay...");
    let _ = dial_and_wait_connection(
        &mut node2,
        *node1.peer_id(),
        node1_relay_addr.clone(),
        Duration::from_secs(10),
    )
    .await?;

    // ФАЗА 4: Завершение теста
    println!("🏁 Фаза 4: Завершение теста...");

    // Очистка ресурсов
    println!("🧹 Выполняем очистку ресурсов...");
    server
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить server узел");
    node1
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node1 узел");
    node2
        .force_shutdown()
        .await
        .expect("❌ Не удалось завершить node2 узел");

    println!("🎉 Тест подключения трех узлов успешно завершен!");
    println!("   - Server: {:?}", server.peer_id());
    println!("   - Node 1: {:?}", node1.peer_id());
    println!("   - Node 2: {:?}", node2.peer_id());
    println!("   - Node1 успешно подключился к server!");

    Ok(())
}

//! Пример использования команды add_autonat_server

use libp2p::{identity, Multiaddr};
use std::time::Duration;
use xnetwork2::node::Node;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запуск теста команды add_autonat_server...");

    // Создаем первый узел (сервер автоната)
    let key1 = identity::Keypair::generate_ed25519();
    let peer_id1 = key1.public().to_peer_id();
    
    let mut node1 = Node::builder().await
        .with_autonat_server()
        .with_keypair(key1.clone())
        .build()
        .await?;
    
    // Запускаем узел
    node1.start().await?;
    
    // Запускаем прослушивание
    let listen_addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()?;
    let actual_addr = node1.commander.listen_and_wait(listen_addr, Duration::from_secs(5)).await?;
    println!("✅ Узел 1 слушает на: {}", actual_addr);

    // Создаем второй узел (клиент автоната)
    let key2 = identity::Keypair::generate_ed25519();
    
    let mut node2 = Node::builder().await
        .with_autonat_client()
        .with_keypair(key2.clone())
        .build()
        .await?;
    
    // Запускаем узел
    node2.start().await?;
    
    // Запускаем прослушивание
    let listen_addr2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()?;
    let actual_addr2 = node2.commander.listen_and_wait(listen_addr2, Duration::from_secs(5)).await?;
    println!("✅ Узел 2 слушает на: {}", actual_addr2);

    // Добавляем первый узел как сервер автоната для второго узла
    println!("🔄 Добавляем узел 1 как сервер автоната для узла 2...");
    node2.add_autonat_server(peer_id1, Some(actual_addr.clone())).await?;
    println!("✅ Узел 1 добавлен как сервер автоната для узла 2");

    // Проверяем статус XRoutes
    let status = node2.get_xroutes_status().await?;
    println!("📊 Статус XRoutes узла 2: {:?}", status);

    // Ждем немного для работы автоната
    println!("⏳ Ждем 10 секунд для работы автоната...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Завершаем работу
    println!("🛑 Завершение работы...");
    node1.force_shutdown().await?;
    node2.force_shutdown().await?;

    println!("✅ Тест завершен успешно!");
    Ok(())
}

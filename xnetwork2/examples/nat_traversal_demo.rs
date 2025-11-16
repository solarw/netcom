//! Демонстрация NAT traversal механизмов в XNetwork2
//!
//! Этот пример показывает, как использовать DCUtR, AutoNAT и relay серверы
//! для обхода NAT и установления прямых соединений между узлами.

use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::node_builder::NodeBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Starting NAT Traversal Demo");
    println!("===============================");

    // Создаем первый узел с включенными всеми механизмами NAT traversal
    println!("\n🛠️ Creating Node 1 with full NAT traversal support...");
    let mut node1 = NodeBuilder::new()
        .with_nat_traversal()  // Включает relay, DCUtR и AutoNAT
        .build()
        .await?;

    println!("✅ Node 1 created with PeerId: {}", node1.peer_id());

    // Создаем второй узел с включенными всеми механизмами NAT traversal
    println!("\n🛠️ Creating Node 2 with full NAT traversal support...");
    let mut node2 = NodeBuilder::new()
        .with_nat_traversal()  // Включает relay, DCUtR и AutoNAT
        .build()
        .await?;

    println!("✅ Node 2 created with PeerId: {}", node2.peer_id());

    // Запускаем оба узла
    println!("\n▶️ Starting both nodes...");
    node1.start().await?;
    node2.start().await?;

    println!("✅ Both nodes started successfully");

    // Даем узлам время для инициализации
    println!("\n⏳ Waiting for nodes to initialize...");
    sleep(Duration::from_secs(2)).await;

    // Показываем статус NAT traversal механизмов
    println!("\n📊 NAT Traversal Status:");
    println!("   - Relay Server: Enabled");
    println!("   - DCUtR (Hole Punching): Enabled");
    println!("   - AutoNAT (NAT Detection): Enabled");

    // Демонстрируем поиск пиров через Kademlia DHT
    println!("\n🔍 Demonstrating peer discovery through Kademlia DHT...");
    
    // Node 1 пытается найти Node 2 через DHT
    match node1.find_peer_addresses(*node2.peer_id(), Duration::from_secs(10)).await {
        Ok(addresses) => {
            if addresses.is_empty() {
                println!("❌ Node 1 could not find Node 2 in DHT");
            } else {
                println!("✅ Node 1 found Node 2 with {} addresses:", addresses.len());
                for addr in &addresses {
                    println!("   - {}", addr);
                }
            }
        }
        Err(e) => {
            println!("❌ Error finding peer: {}", e);
        }
    }

    // Даем узлам время для работы с NAT traversal механизмами
    println!("\n⏳ Letting NAT traversal mechanisms work for 10 seconds...");
    sleep(Duration::from_secs(10)).await;

    // Показываем статус mDNS обнаружения
    println!("\n📡 Checking mDNS discovery...");
    match node1.get_mdns_peers().await {
        Ok(peers) => {
            if peers.is_empty() {
                println!("❌ No mDNS peers discovered (normal in virtual environment)");
            } else {
                println!("✅ mDNS discovered {} peers:", peers.len());
                for (peer_id, addresses) in peers {
                    println!("   - {} with {} addresses", peer_id, addresses.len());
                }
            }
        }
        Err(e) => {
            println!("❌ Error getting mDNS peers: {}", e);
        }
    }

    // Демонстрируем использование relay сервера
    println!("\n🔄 Demonstrating relay functionality...");
    
    // Включаем relay сервер на Node 1
    match node1.commander.enable_relay_server().await {
        Ok(_) => println!("✅ Node 1 relay server enabled"),
        Err(e) => println!("❌ Failed to enable relay server on Node 1: {}", e),
    }

    // Даем время для объявления relay сервера
    sleep(Duration::from_secs(3)).await;

    // Останавливаем узлы
    println!("\n🛑 Stopping nodes...");
    node1.stop().await?;
    node2.stop().await?;

    println!("\n🎉 NAT Traversal Demo completed successfully!");
    println!("=============================================");
    println!("Summary:");
    println!("  - DCUtR: Enabled for hole punching");
    println!("  - AutoNAT: Enabled for NAT type detection");
    println!("  - Relay: Enabled for fallback connections");
    println!("  - mDNS: Enabled for local network discovery");
    println!("  - Kademlia DHT: Enabled for global peer discovery");

    Ok(())
}

//! Программа для генерации .env файла с фиксированными ключами и peer_id
//! Использует libp2p для корректного получения peer_id

use base64::Engine;
use libp2p::identity;
use rand::RngCore;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔑 Генерация .env файла для NAT traversal тестов");
    println!("=================================================\n");

    // Генерируем ключи для всех узлов
    let mut relay_seed = [0u8; 32];
    let mut node1_seed = [0u8; 32];
    let mut node2_seed = [0u8; 32];
    
    rand::thread_rng().fill_bytes(&mut relay_seed);
    rand::thread_rng().fill_bytes(&mut node1_seed);
    rand::thread_rng().fill_bytes(&mut node2_seed);

    // Создаем keypair из seed и получаем peer_id
    let relay_keypair = identity::Keypair::ed25519_from_bytes(relay_seed)
        .expect("❌ Не удалось создать keypair для relay");
    let relay_peer_id = relay_keypair.public().to_peer_id();

    let node1_keypair = identity::Keypair::ed25519_from_bytes(node1_seed)
        .expect("❌ Не удалось создать keypair для node1");
    let node1_peer_id = node1_keypair.public().to_peer_id();

    let node2_keypair = identity::Keypair::ed25519_from_bytes(node2_seed)
        .expect("❌ Не удалось создать keypair для node2");
    let node2_peer_id = node2_keypair.public().to_peer_id();

    // Кодируем ключи в base64
    let relay_key_base64 = base64::engine::general_purpose::STANDARD.encode(relay_seed);
    let node1_key_base64 = base64::engine::general_purpose::STANDARD.encode(node1_seed);
    let node2_key_base64 = base64::engine::general_purpose::STANDARD.encode(node2_seed);

    // Формируем содержимое .env файла
    let env_content = format!(
        "# Тестовые ключи для NAT traversal тестов
# Эти ключи сгенерированы для тестирования

# Relay сервер
RELAY_KEY={}
RELAY_PEER_ID={}

# Node1 (активный узел)
NODE1_KEY={}
NODE1_PEER_ID={}

# Node2 (пассивный узел)
NODE2_KEY={}
NODE2_PEER_ID={}",
        relay_key_base64,
        relay_peer_id,
        node1_key_base64,
        node1_peer_id,
        node2_key_base64,
        node2_peer_id
    );

    // Сохраняем в .env файл
    fs::write(".env", &env_content)?;

    println!("✅ .env файл успешно сгенерирован!");
    println!("📁 Файл: .env");
    println!();
    println!("🔑 Relay сервер:");
    println!("   - Ключ: {}...", &relay_key_base64[..20]);
    println!("   - Peer ID: {}", relay_peer_id);
    println!();
    println!("🔑 Node1 (активный узел):");
    println!("   - Ключ: {}...", &node1_key_base64[..20]);
    println!("   - Peer ID: {}", node1_peer_id);
    println!();
    println!("🔑 Node2 (пассивный узел):");
    println!("   - Ключ: {}...", &node2_key_base64[..20]);
    println!("   - Peer ID: {}", node2_peer_id);
    println!();
    println!("🎯 Использование:");
    println!("   - Для relay: NODE_KEY=${{RELAY_KEY}}");
    println!("   - Для node1: NODE_KEY=${{NODE1_KEY}}");
    println!("   - Для node2: NODE_KEY=${{NODE2_KEY}}");
    println!("   - Peer ID для подключения: ${{NODE2_PEER_ID}}");
    println!("   - Relay Peer ID: ${{RELAY_PEER_ID}}");

    Ok(())
}

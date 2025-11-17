//! Программа key-dump-test - демонстрация выгрузки и загрузки ключей двумя методами:
//! 1. Через 32-байтный Ed25519 seed
//! 2. Через protobuf сериализацию

use base64::Engine;
use libp2p::identity;
use rand::RngCore;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔑 key-dump-test: Демонстрация выгрузки и загрузки ключей");
    println!("==========================================================\n");

    // Метод 1: Работа через 32-байтный seed
    println!("🎯 МЕТОД 1: Работа через 32-байтный Ed25519 seed");
    
    // Генерируем случайный 32-байтный seed
    let mut seed_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed_bytes);
    println!("📏 Сгенерирован seed: {} байт", seed_bytes.len());
    
    // Создаем keypair из seed
    let seed_keypair = identity::Keypair::ed25519_from_bytes(seed_bytes)
        .expect("❌ Не удалось создать keypair из seed");
    let seed_peer_id = seed_keypair.public().to_peer_id();
    println!("✅ PeerId из seed: {}", seed_peer_id);
    
    // Сохраняем seed в файлы
    let seed_base64 = base64::engine::general_purpose::STANDARD.encode(seed_bytes);
    fs::write("seed.bin", &seed_bytes)?;
    fs::write("seed.base64", &seed_base64)?;
    println!("✅ Seed сохранен в файлы:");
    println!("   - seed.bin (бинарный, {} байт)", seed_bytes.len());
    println!("   - seed.base64 (base64): {}", seed_base64);
    
    // Загружаем seed обратно
    let loaded_seed_bytes = fs::read("seed.bin")?;
    let loaded_seed_base64 = fs::read_to_string("seed.base64")?;
    println!("✅ Seed загружен из файлов:");
    println!("   - seed.bin: {} байт", loaded_seed_bytes.len());
    println!("   - seed.base64: {} символов", loaded_seed_base64.len());
    
    // Проверяем корректность загрузки
    assert_eq!(seed_bytes.as_slice(), loaded_seed_bytes.as_slice(), "❌ Бинарный seed не совпадает");
    assert_eq!(seed_base64, loaded_seed_base64, "❌ Base64 seed не совпадает");
    
    // Восстанавливаем keypair из загруженного seed
    let mut loaded_seed_array = [0u8; 32];
    loaded_seed_array.copy_from_slice(&loaded_seed_bytes);
    let recovered_seed_keypair = identity::Keypair::ed25519_from_bytes(loaded_seed_array)
        .expect("❌ Не удалось восстановить keypair из seed");
    let recovered_seed_peer_id = recovered_seed_keypair.public().to_peer_id();
    println!("✅ Ключ восстановлен из seed");
    println!("✅ Восстановленный PeerId: {}", recovered_seed_peer_id);
    
    // Проверяем совпадение PeerId
    assert_eq!(seed_peer_id, recovered_seed_peer_id, "❌ PeerId не совпадают при восстановлении из seed!");
    println!("✅ PeerId совпадают при восстановлении из seed!\n");

    // Метод 2: Работа через protobuf сериализацию
    println!("🎯 МЕТОД 2: Работа через protobuf сериализацию");
    
    // Генерируем новый ключ
    let proto_keypair = identity::Keypair::generate_ed25519();
    let proto_peer_id = proto_keypair.public().to_peer_id();
    println!("✅ Сгенерирован новый ключ");
    println!("✅ PeerId: {}", proto_peer_id);
    
    // Сериализуем ключ в protobuf формат
    let protobuf_bytes = proto_keypair.to_protobuf_encoding()?;
    let protobuf_base64 = base64::engine::general_purpose::STANDARD.encode(&protobuf_bytes);
    println!("📏 Protobuf размер: {} байт", protobuf_bytes.len());
    
    // Сохраняем protobuf в файлы
    fs::write("key.protobuf", &protobuf_bytes)?;
    fs::write("key.protobuf.base64", &protobuf_base64)?;
    println!("✅ Protobuf сохранен в файлы:");
    println!("   - key.protobuf (бинарный, {} байт)", protobuf_bytes.len());
    println!("   - key.protobuf.base64 (base64): {}...", &protobuf_base64[..50]);
    
    // Загружаем protobuf обратно
    let loaded_protobuf_bytes = fs::read("key.protobuf")?;
    let loaded_protobuf_base64 = fs::read_to_string("key.protobuf.base64")?;
    println!("✅ Protobuf загружен из файлов:");
    println!("   - key.protobuf: {} байт", loaded_protobuf_bytes.len());
    println!("   - key.protobuf.base64: {} символов", loaded_protobuf_base64.len());
    
    // Проверяем корректность загрузки
    assert_eq!(protobuf_bytes, loaded_protobuf_bytes, "❌ Бинарный protobuf не совпадает");
    assert_eq!(protobuf_base64, loaded_protobuf_base64, "❌ Base64 protobuf не совпадает");
    
    // Восстанавливаем ключ из protobuf
    let recovered_proto_keypair = identity::Keypair::from_protobuf_encoding(&loaded_protobuf_bytes)?;
    let recovered_proto_peer_id = recovered_proto_keypair.public().to_peer_id();
    println!("✅ Ключ восстановлен из protobuf");
    println!("✅ Восстановленный PeerId: {}", recovered_proto_peer_id);
    
    // Проверяем совпадение PeerId
    assert_eq!(proto_peer_id, recovered_proto_peer_id, "❌ PeerId не совпадают при восстановлении из protobuf!");
    println!("✅ PeerId совпадают при восстановлении из protobuf!\n");

    // Сравнение методов
    println!("🎯 СРАВНЕНИЕ МЕТОДОВ");
    println!("✅ Метод 1 (seed): PeerId = {}", seed_peer_id);
    println!("✅ Метод 2 (protobuf): PeerId = {}", proto_peer_id);
    println!("✅ Оба метода работают корректно!");
    println!("✅ Ключи успешно сохраняются и загружаются!\n");

    // Финальный вывод
    println!("🎉 ПРОГРАММА УСПЕШНО ЗАВЕРШЕНА!");
    println!("==================================");
    println!("✅ Доказана возможность выгрузки и загрузки ключей:");
    println!("   - Метод 1: Через 32-байтный Ed25519 seed");
    println!("   - Метод 2: Через protobuf сериализацию");
    println!("✅ Оба метода дают валидные ключи");
    println!("✅ PeerId сохраняется при всех операциях");
    println!("✅ Ключи корректно сериализуются и десериализуются");

    // Очистка временных файлов
    let _ = fs::remove_file("seed.bin");
    let _ = fs::remove_file("seed.base64");
    let _ = fs::remove_file("key.protobuf");
    let _ = fs::remove_file("key.protobuf.base64");
    println!("✅ Временные файлы удалены");

    Ok(())
}

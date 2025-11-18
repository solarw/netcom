//! Пример использования команд управления режимом Kademlia

use std::time::Duration;
use tokio::time::sleep;
use xnetwork2::{Node, xroutes::types::KadMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запуск демонстрации управления режимом Kademlia...");
    
    // Создаем новую ноду
    let mut node = Node::new().await?;
    println!("✅ Нода создана успешно");
    
    // Получаем Commander для отправки команд
    let commander = node.commander.clone();
    
    // Запускаем ноду
    node.start().await.expect("❌ Не удалось запустить ноду");
    
    // Даем ноде время на инициализацию
    sleep(Duration::from_millis(500)).await;
    
    // Включаем Kademlia если еще не включен
    commander.enable_kad().await?;
    println!("✅ Kademlia включен");
    
    println!("📊 Получаем начальный статус...");
    let status = commander.get_xroutes_status().await?;
    println!("📊 Начальный статус: {:?}", status);
    
    // Тестируем режимы Kademlia
    println!("\n🔧 Тестируем режим Client...");
    commander.set_kad_mode(KadMode::Client).await?;
    let mode = commander.get_kad_mode().await?;
    println!("✅ Текущий режим Kademlia: {}", mode);
    
    println!("\n🔧 Тестируем режим Server...");
    commander.set_kad_mode(KadMode::Server).await?;
    let mode = commander.get_kad_mode().await?;
    println!("✅ Текущий режим Kademlia: {}", mode);
    
    println!("\n🔧 Тестируем режим Auto...");
    commander.set_kad_mode(KadMode::Auto).await?;
    let mode = commander.get_kad_mode().await?;
    println!("✅ Текущий режим Kademlia: {}", mode);
    
    // Проверяем статус после всех изменений
    println!("\n📊 Получаем финальный статус...");
    let final_status = commander.get_xroutes_status().await?;
    println!("📊 Финальный статус: {:?}", final_status);
    
    // Даем ноде поработать немного
    println!("⏳ Нода работает с командами... (ожидание 2 секунды)");
    sleep(Duration::from_secs(2)).await;
    
    // Graceful shutdown
    println!("🛑 Останавливаем ноду...");
    commander.shutdown().await?;
    
    // Ожидаем завершения фоновой задачи
    node.wait_for_shutdown().await?;
    
    println!("✅ Демонстрация завершена успешно!");
    Ok(())
}

//! Пример использования команд управления режимами Kademlia

use xnetwork2::behaviours::xroutes::types::KadMode;
use xnetwork2::Node;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запуск теста управления режимами Kademlia...");

    // Создаем узел с отключенными компонентами для теста
    let mut node = Node::builder()
        .await
        .build()
        .await?;

    // Запускаем узел
    node.start().await?;
    println!("✅ Узел запущен");

    // Включаем Kademlia
    println!("🔄 Включаем Kademlia...");
    node.enable_kad().await?;
    println!("✅ Kademlia включена");

    // Получаем текущий режим
    println!("🔄 Получаем текущий режим Kademlia...");
    let current_mode = node.get_kad_mode().await?;
    println!("✅ Текущий режим Kademlia: {}", current_mode);

    // Устанавливаем режим Client
    println!("🔄 Устанавливаем режим Client...");
    node.set_kad_mode(KadMode::Client).await?;
    println!("✅ Режим установлен: Client");

    // Проверяем что режим изменился
    let new_mode = node.get_kad_mode().await?;
    println!("✅ Проверяем новый режим: {}", new_mode);
    assert_eq!(new_mode, KadMode::Client, "Режим должен быть Client");

    // Устанавливаем режим Server
    println!("🔄 Устанавливаем режим Server...");
    node.set_kad_mode(KadMode::Server).await?;
    println!("✅ Режим установлен: Server");

    // Проверяем что режим изменился
    let new_mode = node.get_kad_mode().await?;
    println!("✅ Проверяем новый режим: {}", new_mode);
    assert_eq!(new_mode, KadMode::Server, "Режим должен быть Server");

    // Устанавливаем режим Auto (в libp2p-kad это None)
    println!("🔄 Устанавливаем режим Auto...");
    node.set_kad_mode(KadMode::Auto).await?;
    println!("✅ Режим установлен: Auto");

    // Примечание: В libp2p-kad мы не можем определить Auto режим через mode(),
    // поэтому после установки Auto режим может вернуться как Client или Server
    let new_mode = node.get_kad_mode().await?;
    println!("✅ Проверяем текущий режим: {}", new_mode);
    println!("ℹ️  В libp2p-kad Auto режим не определяется через mode(), только Client/Server");

    // Получаем статус XRoutes для проверки
    println!("🔄 Получаем статус XRoutes...");
    let status = node.get_xroutes_status().await?;
    println!("✅ Статус XRoutes:");
    println!("   - Kademlia включена: {}", status.kad_enabled);
    println!("   - Режим Kademlia: {:?}", status.kad_mode);

    // Останавливаем узел
    println!("🛑 Останавливаем узел...");
    node.stop().await?;
    println!("✅ Узел остановлен");

    println!("🎉 Тест управления режимами Kademlia завершен успешно!");
    Ok(())
}

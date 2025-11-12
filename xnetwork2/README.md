# XNetwork2 - P2P сеть на Rust с архитектурой command-swarm

[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org)
[![libp2p](https://img.shields.io/badge/libp2p-0.56-blue.svg)](https://libp2p.io)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**XNetwork2** - это переработанная версия P2P сети на Rust, использующая архитектуру command-swarm для структурированного управления libp2p swarm через отдельные handlers. Проект предоставляет надежную основу для создания распределенных приложений с поддержкой аутентификации и потоковой передачи данных.

## 🚀 Основные возможности

- **Command-based архитектура** - все операции через типизированные команды
- **Встроенная аутентификация** через xauth с Proof of Representation
- **Потоковая передача данных** через xstream с двунаправленными потоками
- **Асинхронные операции** на базе tokio с полной неблокируемостью
- **Система событий** с поддержкой множественных подписчиков через broadcast каналы
- **Graceful shutdown** - корректное завершение работы с освобождением ресурсов
- **Модульная архитектура** - отдельные обработчики для каждого protocol behaviour
- **Типобезопасность** - строгая типизация команд и событий

## 🏗️ Архитектура

```
XNetwork2
├── Node (управление жизненным циклом)
│   ├── SwarmLoop (фоновый цикл обработки)
│   ├── Commander (API для команд)
│   └── Event System (broadcast события)
├── Behaviour Handlers
│   ├── IdentifyHandler (идентификация пиров)
│   ├── PingHandler (проверка доступности)
│   ├── XAuthHandler (аутентификация)
│   └── XStreamHandler (потоковая передача)
└── Swarm Handler
    └── XNetworkSwarmHandler (управление соединениями)
```

## 📦 Быстрый старт

### Установка

```toml
[dependencies]
xnetwork2 = { path = "../xnetwork2" }
```

### Минимальный пример

```rust
use xnetwork2::Node;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Создание ноды
    let mut node = Node::new().await?;
    println!("✅ Нода создана с PeerId: {}", node.peer_id());
    
    // Подписка на события
    let mut events = node.subscribe();
    
    // Запуск ноды
    node.start().await?;
    println!("✅ Нода запущена");
    
    // Отправка команд через Commander
    node.commander.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse()?).await?;
    println!("✅ Команда listen_on выполнена");
    
    // Graceful shutdown
    node.commander.shutdown().await?;
    node.wait_for_shutdown().await?;
    println!("✅ Нода корректно завершена");
    
    Ok(())
}
```

## 📚 API документация

### Node

Основной класс для управления P2P нодой:

```rust
// Создание ноды
let mut node = Node::new().await?;

// Запуск ноды
node.start().await?;

// Подписка на события
let mut events = node.subscribe();

// Получение PeerId
let peer_id = node.peer_id();

// Graceful shutdown
node.commander.shutdown().await?;
node.wait_for_shutdown().await?;
```

### Commander

API для отправки команд ноде:

```rust
// Прослушивание адреса
commander.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse()?).await?;

// Echo команда (для тестирования)
let response = commander.echo("Hello, XNetwork2!".to_string()).await?;

// Получение состояния сети
let state = commander.get_network_state().await?;

// Завершение работы
commander.shutdown().await?;
```

### События (NodeEvent)

Система событий для отслеживания состояния ноды:

```rust
use xnetwork2::node_events::NodeEvent;

let mut events = node.subscribe();

while let Ok(event) = events.recv().await {
    match event {
        NodeEvent::NewListenAddr { address } => {
            println!("📡 Нода начала прослушивать: {}", address);
        }
        NodeEvent::ConnectionEstablished { peer_id } => {
            println!("🔗 Установлено соединение с: {}", peer_id);
        }
        NodeEvent::ConnectionClosed { peer_id } => {
            println!("🔌 Соединение закрыто с: {}", peer_id);
        }
        NodeEvent::ExpiredListenAddr { address } => {
            println!("❌ Адрес прослушивания истек: {}", address);
        }
        _ => {
            println!("📊 Получено событие: {:?}", event);
        }
    }
}
```

## 🔧 Расширенное использование

### Полный пример с событиями

```rust
use xnetwork2::{Node, node_events::NodeEvent};
use libp2p::Multiaddr;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Запуск XNetwork2 с системой событий...");

    // Создание ноды
    let mut node = Node::new().await?;
    println!("✅ Нода создана с PeerId: {}", node.peer_id());

    // Подписка на события
    let mut events = node.subscribe();

    // Задача для обработки событий
    let events_task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                NodeEvent::NewListenAddr { address } => {
                    println!("📡 Нода начала прослушивать: {}", address);
                }
                NodeEvent::ConnectionEstablished { peer_id } => {
                    println!("🔗 Установлено соединение с: {}", peer_id);
                }
                NodeEvent::ConnectionClosed { peer_id } => {
                    println!("🔌 Соединение закрыто с: {}", peer_id);
                }
                _ => {}
            }
        }
    });

    // Запуск ноды
    node.start().await?;

    // Команда прослушивания
    node.commander.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse()?).await?;

    // Работа ноды
    sleep(Duration::from_secs(5)).await;

    // Завершение работы
    node.commander.shutdown().await?;
    node.wait_for_shutdown().await?;

    // Ожидание завершения задачи событий
    events_task.abort();
    
    println!("✅ Пример успешно завершен!");
    Ok(())
}
```

### Тестирование соединения между двумя нодами

```rust
use xnetwork2::Node;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_two_nodes_connection() {
    // Создание двух нод
    let mut node1 = Node::new().await.expect("Не удалось создать ноду 1");
    let mut node2 = Node::new().await.expect("Не удалось создать ноду 2");
    
    // Запуск обеих нод
    node1.start().await.expect("Не удалось запустить ноду 1");
    node2.start().await.expect("Не удалось запустить ноду 2");
    
    // Здесь можно добавить логику соединения нод
    // и проверки передачи данных
    
    // Graceful shutdown
    node1.commander.shutdown().await.expect("Не удалось завершить ноду 1");
    node2.commander.shutdown().await.expect("Не удалось завершить ноду 2");
    
    node1.wait_for_shutdown().await.expect("Ошибка завершения ноды 1");
    node2.wait_for_shutdown().await.expect("Ошибка завершения ноды 2");
}
```

## 🧪 Тестирование

Проект включает комплексные интеграционные тесты:

```bash
# Запуск всех тестов
cd xnetwork2
cargo test

# Запуск конкретного теста
cargo test test_node_lifecycle_in_5_seconds
cargo test test_two_nodes_connection
cargo test test_two_nodes_xstream_data_transfer
cargo test test_xauth_mutual_authentication
```

### Доступные тесты:

- **`test_node_lifecycle_in_5_seconds`** - полный жизненный цикл ноды за 5 секунд
- **`test_two_nodes_connection`** - соединение между двумя нодами
- **`test_two_nodes_xstream_data_transfer`** - передача данных через xstream
- **`test_xauth_mutual_authentication`** - взаимная аутентификация через xauth

## 🔍 Отладка

Для отладки включите логирование:

```rust
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Инициализация логирования
    tracing_subscriber::fmt::init();
    
    // Остальной код...
    Ok(())
}
```

## 📁 Структура проекта

```
xnetwork2/
├── src/
│   ├── lib.rs                    # Основной модуль и re-exports
│   ├── node.rs                   # Управление жизненным циклом ноды
│   ├── commander.rs              # API для отправки команд
│   ├── node_events.rs            # Система событий
│   ├── main_behaviour.rs         # Объединенный behaviour
│   ├── swarm_handler.rs          # Обработчик swarm-level операций
│   ├── swarm_commands.rs         # Swarm-level команды
│   └── behaviours/               # Behaviour handlers
│       ├── mod.rs               # Экспорт всех handlers
│       ├── identify/            # Identify handler
│       ├── ping/               # Ping handler
│       ├── xauth/              # XAuth adapter
│       └── xstream/            # XStream adapter
├── examples/
│   └── command_demo.rs          # Полный пример использования
└── tests/
    ├── node_lifecycle_integration.rs
    ├── two_nodes_connection.rs
    ├── two_nodes_xstream_data_transfer.rs
    └── xauth_mutual_authentication.rs
```

## 🔗 Зависимости

- **libp2p 0.56** - базовая P2P библиотека
- **command-swarm** - архитектура управления swarm
- **xauth** - аутентификация и Proof of Representation
- **xstream** - потоковая передача данных
- **tokio** - асинхронность
- **tracing** - логирование

## 🤝 Вклад в проект

1. Форкните репозиторий
2. Создайте ветку для новой функциональности (`git checkout -b feature/amazing-feature`)
3. Зафиксируйте изменения (`git commit -m 'Add amazing feature'`)
4. Отправьте в ветку (`git push origin feature/amazing-feature`)
5. Создайте Pull Request

## 📄 Лицензия

Этот проект распространяется под лицензией MIT. Подробности см. в файле [LICENSE](LICENSE).

## 📞 Поддержка

- Создайте issue в репозитории для багов и предложений
- Для вопросов используйте discussions

---

**XNetwork2** - надежная основа для ваших P2P приложений на Rust! 🚀

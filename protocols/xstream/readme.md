# XStream - Advanced P2P Stream Protocol

XStream - это протокол расширенных потоков для удобной сетевой коммуникации в стиле RPC с надежной обработкой ошибок. Основное преимущество - удобная обработка ошибок за счет создания дополнительного потока ошибок, который гарантирует своевременное определение ошибок, отправленных сервером.

## 🚀 Key Features

- **Надежная обработка ошибок** - отдельный поток для передачи ошибок
- **Механизм принятия решений** - контроль над входящими соединениями
- **Асинхронные операции** - неблокирующие операции чтения/записи
- **Гарантированная доставка** - контроль состояния потоков
- **Гибкие сценарии использования** - RPC, streaming, messaging

## 📋 Use Cases

Используя XStream можно реализовать различные варианты взаимодействия:

- **Стандартный запрос-ответ** - классический RPC паттерн
- **Одно или двусторонний обмен потоком байтов** - файловый обмен, стриминг
- **Одно или двусторонний обмен сообщениями** - чат, уведомления

## 🏗️ Architecture

XStream состоит из двух потоков `libp2p::Stream`:
- **Основной поток** - двунаправленный для данных
- **Поток ошибок** - однонаправленный от сервера к клиенту

## ⚡ Quick Start

### Installation

Добавьте зависимость в ваш `Cargo.toml`:

```toml
[dependencies]
xstream = { path = "protocols/xstream" }
```

### Basic Usage

```rust
use xstream::behaviour::XStreamNetworkBehaviour;
use libp2p::{Swarm, swarm::SwarmBuilder, identity, PeerId};
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    // Создание идентификатора узла
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    
    // Создание транспорта
    let transport = libp2p::development_transport(local_key).await.unwrap();
    
    // Создание поведения XStream
    let behaviour = XStreamNetworkBehaviour::new();
    
    // Создание Swarm
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    
    // Запуск прослушивания
    swarm.listen_on("/memory/0".parse().unwrap()).unwrap();
    
    println!("XStream node started with peer ID: {}", local_peer_id);
}
```

### Opening a Stream and Data Exchange

```rust
use xstream::behaviour::XStreamNetworkBehaviour;
use xstream::xstream::XStream;
use libp2p::{PeerId, Multiaddr};
use tokio::sync::oneshot;

async fn open_stream_and_exchange_data(
    swarm: &mut Swarm<XStreamNetworkBehaviour>,
    peer_id: PeerId
) -> Result<(), Box<dyn std::error::Error>> {
    // Открытие потока
    let (response_sender, response_receiver) = oneshot::channel();
    swarm.behaviour_mut().open_stream(peer_id, response_sender).await;
    
    // Получение XStream
    let xstream = response_receiver.await??;
    
    // Запись данных
    let data = b"Hello, XStream!";
    xstream.write_all(data.to_vec()).await?;
    xstream.flush().await?;
    
    // Чтение ответа (используем read_to_end для простоты)
    let response = xstream.read_to_end().await?;
    
    println!("Received: {}", String::from_utf8_lossy(&response));
    
    // Закрытие потока
    xstream.close().await?;
    
    Ok(())
}
```

## 🔧 API Reference

### Основные методы работы с данными:

```rust
// Чтение данных
async fn read(&self, buf: &mut [u8]) -> Result<usize, XStreamError>;
async fn read_exact(&self, buf: &mut [u8]) -> Result<(), XStreamError>;
async fn read_to_end(&self) -> Result<Vec<u8>, XStreamError>;

// Запись данных
async fn write(&self, buf: &[u8]) -> Result<usize, XStreamError>;
async fn write_all(&self, buf: &[u8]) -> Result<(), XStreamError>;
async fn flush(&self) -> Result<(), XStreamError>;

// Управление потоком
async fn write_eof(&self) -> Result<(), XStreamError>;
async fn close(&self) -> Result<(), XStreamError>;
```

### Работа с ошибками

```rust
// Запись ошибки
async fn error_write(&self, error_data: &[u8], with_data_flush: bool) -> Result<(), XStreamError>;

// Чтение ошибки
async fn error_read(&self) -> Result<Vec<u8>, XStreamError>;
```

**Логика работы при полученной ошибке:**
- Полученная ошибка сохраняется в кэше
- Повторный вызов `error_read()` вернет сохраненную ошибку
- По потоку ошибок может прийти только один блок данных, заканчивающийся EOF
- Нельзя записать ошибку дважды

## 🎯 Advanced Features

### Механизм принятия решений о входящих соединениях

XStream предоставляет гибкий механизм контроля над входящими соединениями:

```rust
use xstream::events::{IncomingConnectionApprovePolicy, InboundUpgradeDecision};

// Автоматическое одобрение всех соединений
let behaviour = XStreamNetworkBehaviour::new(); // AutoApprove по умолчанию

// Или ручное управление через события
let behaviour = XStreamNetworkBehaviour::new_with_policy(
    IncomingConnectionApprovePolicy::ApproveViaEvent
);

// Обработка событий запросов
match swarm.next().await {
    Some(SwarmEvent::Behaviour(XStreamEvent::InboundUpgradeRequest {
        peer_id,
        connection_id,
        response_sender,
    })) => {
        // Принятие решения
        let decision = if should_accept_connection(&peer_id) {
            InboundUpgradeDecision::Approved
        } else {
            InboundUpgradeDecision::Rejected("Peer not allowed".to_string())
        };
        
        response_sender.send(decision).ok();
    }
    // ... другие события
}
```

### Политики принятия решений

- **`AutoApprove`** - автоматическое одобрение всех входящих апгрейдов
- **`ApproveViaEvent`** - передача события в Swarm для пользовательской обработки

## 🔄 State Management

Самая сложная и интересная часть XStream:

- Одновременно может работать только одна операция записи или чтения данных (гарантируется применяемым `Mutex`)
- Параллельно проверяется получение ошибок
- Если ошибка возникает в момент блокирующей операции чтения/записи, возвращается ошибка и выставляются нужные статусы

## 🧪 Testing

Проект включает 170+ тестов, покрывающих все аспекты функциональности:

```bash
cd protocols/xstream
cargo test

# Запуск только тестов принятия решений
cargo test inbound_upgrade
```

## 📚 Examples

### Базовый пример использования

Полный рабочий пример с QUIC транспортом и правильной архитектурой swarm loop:

```bash
cargo run --example basic_usage
```

```rust
use xstream::behaviour::XStreamNetworkBehaviour;
use xstream::events::XStreamEvent;
use libp2p::{identity, quic, Swarm, SwarmEvent, Multiaddr, PeerId};
use tokio::sync::{oneshot, mpsc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Создание двух узлов с QUIC транспортом
    let (mut client_swarm, client_peer_id) = create_quic_swarm().await?;
    let (mut server_swarm, server_peer_id) = create_quic_swarm().await?;
    
    // Запуск сервера и клиента в параллельных задачах
    // Полная реализация в examples/basic_usage.rs
}
```

### Механизм принятия решений о входящих потоках

Пример с авторизацией пиров и отправкой ошибок при отклонении:

```bash
cargo run --example inbound_decision
```

```rust
use xstream::behaviour::XStreamNetworkBehaviour;
use xstream::events::{XStreamEvent, InboundUpgradeDecision, IncomingConnectionApprovePolicy};
use libp2p::{identity, quic, Swarm, SwarmEvent, PeerId};
use std::collections::HashSet;

// Важно: для работы механизма принятия решений используйте политику ApproveViaEvent
let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
    .with_tokio()
    .with_other_transport(|_key| quic_transport)
    .expect("Не удалось создать QUIC транспорт")
    .with_behaviour(|_key| {
        XStreamNetworkBehaviour::new_with_policy(
            IncomingConnectionApprovePolicy::ApproveViaEvent
        )
    })
    .expect("Не удалось создать XStream поведение")
    .build();

async fn handle_inbound_upgrade(
    swarm: &mut Swarm<XStreamNetworkBehaviour>,
    allowed_peers: &HashSet<PeerId>
) {
    while let Some(event) = swarm.next().await {
        match event {
            SwarmEvent::Behaviour(XStreamEvent::InboundUpgradeRequest {
                peer_id,
                connection_id,
                response_sender,
            }) => {
                // Механизм принятия решения
                let decision = if allowed_peers.contains(&peer_id) {
                    InboundUpgradeDecision::Approved
                } else {
                    InboundUpgradeDecision::Rejected("Peer not authorized".to_string())
                };
                
                response_sender.send(decision).ok();
            }
            _ => {}
        }
    }
}
```

**Важно:** По умолчанию XStream использует политику `AutoApprove`, которая автоматически одобряет все входящие соединения. Для активации механизма принятия решений необходимо явно установить политику `ApproveViaEvent` при создании поведения.

### Простой RPC сервер

```rust
use xstream::{XStreamNetworkBehaviour, XStream};
use libp2p::{Swarm, SwarmEvent};

async fn rpc_server(mut swarm: Swarm<XStreamNetworkBehaviour>) {
    while let Some(event) = swarm.next().await {
        match event {
            SwarmEvent::Behaviour(XStreamEvent::IncomingStream { stream }) => {
                tokio::spawn(async move {
                    handle_rpc_request(stream).await;
                });
            }
            _ => {}
        }
    }
}

async fn handle_rpc_request(mut stream: XStream) -> Result<(), Box<dyn std::error::Error>> {
    // Чтение запроса
    let request = stream.read_to_end().await?;
    
    // Обработка запроса
    let response = process_request(&request).await?;
    
    // Отправка ответа
    stream.write_all(&response).await?;
    stream.flush().await?;
    
    Ok(())
}
```

### Потоковая передача файлов

```rust
async fn stream_file(mut stream: XStream, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file_data = std::fs::read(file_path)?;
    
    // Отправка файла чанками
    for chunk in file_data.chunks(1024) {
        stream.write_all(chunk).await?;
    }
    
    stream.write_eof().await?;
    Ok(())
}
```

## 🔗 Integration

XStream легко интегрируется с существующими libp2p приложениями:

```rust
use libp2p::swarm::NetworkBehaviour;

#[derive(NetworkBehaviour)]
struct MyAppBehaviour {
    xstream: XStreamNetworkBehaviour,
    identify: libp2p::identify::Behaviour,
    // ... другие поведения
}
```

## 📊 Performance

- **Низкая задержка** - асинхронные операции
- **Высокая пропускная способность** - эффективная буферизация
- **Минимальные накладные расходы** - оптимизированная архитектура

## 🤝 Contributing

Мы приветствуем вклад в развитие XStream! Пожалуйста, убедитесь, что все тесты проходят перед отправкой пул-реквеста.

## 📄 License

XStream распространяется под лицензией проекта NetCom.

---

**XStream** - надежная P2P коммуникация с продвинутой обработкой ошибок и гибким контролем соединений.

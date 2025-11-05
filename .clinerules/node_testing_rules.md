# Правила тестирования нод NetCom

Этот файл определяет правильные подходы к тестированию нод NetCom с соблюдением событийной модели и предотвращением потери событий.

## Основные принципы

### 1. Правильная последовательность операций
**Создание ноды → Создание обработчиков событий → Запуск обработки событий → Операции с командой → Ожидание событий**

Все операции с командой (commander) должны выполняться ПОСЛЕ того как запущен цикл обработки событий.

### 2. Использование обработчиков событий
Всегда используйте обработчики из `utils::event_handlers` для ожидания конкретных событий.

### 3. Явное ожидание событий вместо предположений о состоянии
Предпочитайте явное ожидание событий (например, `ListeningOnAddress`) вместо получения состояния через команды (например, `get_network_state()`).

### 4. Таймауты и обработка ошибок
Все асинхронные операции должны иметь разумные таймауты для предотвращения зависаний.

## Эталонный тест

Файл `xnetwork/tests/xauth/test_auth_integration.rs` является эталоном правильного тестирования:

```rust
#[tokio::test]
async fn test_auth_integration_basic() {
    println!("🧪 Testing basic XAuth integration in XNetwork");
    
    let test_timeout = Duration::from_secs(15);
    
    let result = tokio::time::timeout(test_timeout, async {
        use libp2p::identity;
        use xauth::por::por::ProofOfRepresentation;
        use xnetwork::{NodeBuilder, XRoutesConfig, Commander, NetworkEvent};
        use crate::test_utils::create_node;
        
        // Создаем серверную ноду
        let (server_commander, mut server_events, server_handle, server_peer_id) = 
            create_node().await.expect("Failed to create server node");
        
        // Создаем клиентскую ноду
        let (client_commander, mut client_events, client_handle, client_peer_id) = 
            create_node().await.expect("Failed to create client node");
        
        // Создаем обработчики для сервера
        let (server_listening_rx, mut server_listening_handler) = 
            crate::utils::event_handlers::create_listening_address_handler();
        let (server_connected_rx, mut server_connected_handler) = 
            crate::utils::event_handlers::create_peer_connected_handler(client_peer_id);
        let (server_auth_success_rx, mut server_auth_success_handler) = 
            crate::utils::event_handlers::create_mutual_auth_handler(client_peer_id);
        let mut server_por_approve_handler = 
            crate::utils::event_handlers::create_handler_submit_por_verification(server_commander.clone());
        
        // Создаем обработчики для клиента
        let (client_connected_rx, mut client_connected_handler) = 
            crate::utils::event_handlers::create_peer_connected_handler(server_peer_id);
        let (client_auth_success_rx, mut client_auth_success_handler) = 
            crate::utils::event_handlers::create_mutual_auth_handler(server_peer_id);
        let mut client_por_approve_handler = 
            crate::utils::event_handlers::create_handler_submit_por_verification(client_commander.clone());
        
        // ✅ ПРАВИЛЬНО: сначала запускаем обработку событий
        println!("🔄 Запускаем обработку событий сервера...");
        let server_events_task = tokio::spawn(async move {
            while let Some(event) = server_events.recv().await {
                println!("📡 SERVER EVENT: {:?}", event);
                // обрабатывает событие прослушивания
                server_listening_handler(&event);
                // обрабатывает событие подключения
                server_connected_handler(&event);
                // обрабатывает событие авторизация пройдена
                server_auth_success_handler(&event);
                // обрабатывает событие por request, и всегда одобряет
                server_por_approve_handler(&event);
            }
        });
        
        println!("🔄 Запускаем обработку событий клиента...");
        let client_events_task = tokio::spawn(async move {
            while let Some(event) = client_events.recv().await {
                println!("📡 CLIENT EVENT: {:?}", event);
                client_connected_handler(&event);
                client_auth_success_handler(&event);
                client_por_approve_handler(&event);
            }
        });
        
        // ✅ ПРАВИЛЬНО: потом операции с командой
        
        // Сервер начинает слушать
        println!("🔄 Сервер начинает прослушивание...");
        server_commander.listen_port(Some("127.0.0.1".to_string()), 0).await
            .expect("Failed to start server listening");
        
        // ✅ ПРАВИЛЬНО: ожидаем события прослушивания
        println!("⏳ Ожидаем события прослушивания...");
        let server_listening = tokio::time::timeout(Duration::from_secs(10), server_listening_rx).await
            .expect("Server should start listening within timeout")
            .expect("Failed to get listening address");
        
        println!("✅ Сервер запущен на адресе: {}", server_listening);
        
        let server_addr = server_listening;
        
        // Клиент подключается к серверу
        println!("🔄 Клиент подключается к серверу...");
        client_commander.connect(server_addr).await
            .expect("Failed to connect client to server");
        
        // ✅ ПРАВИЛЬНО: ожидаем события в теле теста
        println!("⏳ Ожидаем события подключения...");
        let client_connected = tokio::time::timeout(Duration::from_secs(10), client_connected_rx).await;
        let server_connected = tokio::time::timeout(Duration::from_secs(10), server_connected_rx).await;
        
        assert!(client_connected.is_ok(), "Client should connect to server");
        assert!(server_connected.is_ok(), "Server should see client connection");
        
        println!("✅ Оба узла получили события подключения");
        
        // ✅ ПРАВИЛЬНО: ожидаем события аутентификации
        println!("⏳ Ожидаем успешной аутентификации...");
        let client_auth_success = tokio::time::timeout(Duration::from_secs(10), client_auth_success_rx).await;
        let server_auth_success = tokio::time::timeout(Duration::from_secs(10), server_auth_success_rx).await;
        
        assert!(client_auth_success.is_ok(), "Client should complete authentication with server");
        assert!(server_auth_success.is_ok(), "Server should complete authentication with client");
        
        println!("✅ Оба узла завершили взаимную аутентификацию");
        
        // Проверяем статус аутентификации
        let is_client_authenticated = client_commander
            .is_peer_authenticated(server_peer_id).await
            .expect("Failed to check client authentication");
        
        let is_server_authenticated = server_commander
            .is_peer_authenticated(client_peer_id).await
            .expect("Failed to check server authentication");
        
        println!("🔐 СТАТУС АУТЕНТИФИКАЦИИ:");
        println!("   Client → Server: {}", is_client_authenticated);
        println!("   Server → Client: {}", is_server_authenticated);
        
        assert!(is_client_authenticated, "Client should be authenticated to server");
        assert!(is_server_authenticated, "Server should be authenticated to client");
        
        // Очистка
        client_commander.shutdown().await.expect("Failed to shutdown client");
        server_commander.shutdown().await.expect("Failed to shutdown server");
        
        // Ждем завершения задач
        let _ = tokio::join!(server_handle, client_handle, server_events_task, client_events_task);
        
        println!("✅ XAuth integration test completed!");
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ XAuth integration test completed"),
        Err(_) => panic!("⏰ XAuth integration test timed out ({}s)", test_timeout.as_secs()),
    }
}
```

## Ключевые элементы эталонного теста

### 1. Правильная последовательность
- Создание нод
- Создание обработчиков событий
- **Запуск обработки событий**
- **Операции с командой**
- Ожидание событий
- Проверки

### 2. Использование обработчиков
```rust
// Создание обработчиков для ожидания конкретных событий
let (connected_rx, mut connected_handler) = 
    create_peer_connected_handler(target_peer_id);
let (auth_rx, mut auth_handler) = 
    create_mutual_auth_handler(target_peer_id);
let mut por_approve_handler = 
    create_handler_submit_por_verification(commander.clone());
```

### 3. Ожидание событий с таймаутами
```rust
// Ожидание с таймаутом
let connected = tokio::time::timeout(Duration::from_secs(10), connected_rx).await;
assert!(connected.is_ok(), "Should receive connection event");
```

### 4. Логирование и отладка
- Все события логируются
- Каждый этап сопровождается информационными сообщениями
- Ошибки явно обрабатываются

## Доступные обработчики

### Базовые обработчики событий
```rust
use crate::utils::event_handlers;

// Начало прослушивания адреса
let (listening_rx, mut listening_handler) = 
    event_handlers::create_listening_address_handler();

// Подключение пира
let (connected_rx, mut connected_handler) = 
    event_handlers::create_peer_connected_handler(peer_id);

// Отключение пира  
let (disconnected_rx, mut disconnected_handler) = 
    event_handlers::create_peer_disconnected_handler(peer_id);

// Взаимная аутентификация
let (auth_rx, mut auth_handler) = 
    event_handlers::create_mutual_auth_handler(peer_id);

// Автоматическое одобрение POR
let mut por_handler = 
    event_handlers::create_handler_submit_por_verification(commander.clone());
```

## Запрещенные подходы

### ❌ НЕПРАВИЛЬНО
```rust
// Операции ДО обработки событий
commander.connect(addr).await?;
let events_task = tokio::spawn(async move {
    while let Some(event) = events.recv().await {
        // События подключения уже потеряны!
    }
});
```

### ❌ НЕПРАВИЛЬНО
```rust
// Использование временных TestNode для ожидания
let mut temp_node = TestNode::new().await?;
temp_node.wait_for_connection(peer_id, timeout).await;
```

### ❌ НЕПРАВИЛЬНО
```rust
// Отсутствие таймаутов
let _ = connected_rx.await; // Может зависнуть навсегда
```

## Рекомендуемые таймауты

- **Базовые операции**: 5-10 секунд
- **Сетевые операции**: 10-15 секунд  
- **Аутентификация**: 15-20 секунд
- **Полный тест**: 30 секунд

## Строгая обработка ошибок

### Запрет на "мягкие" обработки
- **❌ НЕПРАВИЛЬНО**: `println!("⚠️ Не удалось выполнить операцию")`
- **✅ ПРАВИЛЬНО**: `panic!("❌ Не удалось выполнить операцию: {} - функциональность не работает!", e)`

### Примеры правильного подхода
```rust
// ❌ НЕПРАВИЛЬНО
match operation_result {
    Ok(_) => println!("✅ Операция выполнена"),
    Err(e) => println!("⚠️ Не удалось выполнить операцию: {}", e),
}

// ✅ ПРАВИЛЬНО
match operation_result {
    Ok(_) => println!("✅ Операция выполнена"),
    Err(e) => panic!("❌ Не удалось выполнить операцию: {} - функциональность не работает!", e),
}
```

## Проверка целостности данных

### Требования к тестам передачи данных
- **❌ НЕПРАВИЛЬНО**: отправить данные и не проверить их получение
- **✅ ПРАВИЛЬНО**: отправить данные, получить их и проверить совпадение с отправленными

### Пример проверки данных
```rust
// Отправка тестовых данных
let test_data = b"Hello, NetCom!";
stream.write_all(test_data).await.expect("Failed to send data");

// Получение и проверка данных
let mut received = vec![0; test_data.len()];
stream.read_exact(&mut received).await.expect("Failed to receive data");
assert_eq!(test_data, &received[..], "Отправленные и полученные данные не совпадают!");
```

### Дополнительные требования
- **Генерация уникальных тестовых данных** - для предотвращения ложных срабатываний
- **Проверка граничных случаев** - пустые данные, максимальные размеры, специальные символы
- **Логирование операций передачи** - для отладки проблем с данными

## Чеклист валидации теста

- [ ] **Обработка событий запускается ДО любых операций с командой**
- [ ] **Используются обработчики из utils::event_handlers**
- [ ] **Все асинхронные операции имеют таймауты**
- [ ] **События логируются для отладки**
- [ ] **Ресурсы корректно освобождаются при завершении**
- [ ] **Тест завершается в разумные сроки (до 30 секунд)**
- [ ] **Используется явное ожидание событий вместо получения состояния**
- [ ] **Запрещены "мягкие" обработки ошибок - используется panic!**
- [ ] **Проверяется целостность данных в тестах передачи**

## Отладка проблем

### Симптомы:
- Тесты проходят, но не видят ожидаемые события
- Аутентификация не завершается, хотя соединение установлено
- События приходят с задержкой или не приходят вовсе

### Решение:
1. **Проверить последовательность** - обработка событий должна запускаться первым делом
2. **Логировать все события** - убедиться, что события действительно генерируются
3. **Не увеличить таймауты** - обычно операциям хватает секунды или долей секунды что бы завершиться, если тест не выполняется за 15 секунд, скорее всего есть проблемы в логике теста и или реализации проекта
4. **Проверить обработчики** - убедиться, что обработчики правильно настроены

# Политика таймаутов для тестов NetCom

## Обзор

Этот документ описывает политику таймаутов для всех тестов проекта NetCom. Все тесты должны иметь явные таймауты для предотвращения зависаний.

## Стандарты таймаутов

### Уровни таймаутов

1. **Базовые тесты (CORE)**: 10 секунд
   - Создание ноды
   - Базовая функциональность
   - Диагностика

2. **Сетевые тесты (CONNECTIONS)**: 15-20 секунд  
   - Установление соединений
   - Управление соединениями
   - Таймауты подключения

3. **Сложные тесты (RELAY/DISCOVERY)**: 30 секунд
   - Relay функциональность
   - DHT поиск
   - Bootstrap серверы

### Реализация таймаутов

Все тесты должны использовать явные таймауты:

```rust
#[tokio::test]
async fn test_example() {
    let test_timeout = Duration::from_secs(10);
    
    let result = tokio::time::timeout(test_timeout, async {
        // Тестовый код
        // ...
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Test completed"),
        Err(_) => panic!("⏰ Test timed out ({}s)", test_timeout.as_secs()),
    }
}
```

## Конфигурация таймаутов

### Cargo.toml (устаревший способ)
В Cargo.toml добавлена настройка дефолтного таймаута:

```toml
[package.metadata.cargo-test]
test-timeout = 20
```

**Важно**: Эта настройка не заменяет явные таймауты в коде тестов!

### Nextest.toml (рекомендуемый способ)
Создан файл `nextest.toml` с продвинутой конфигурацией таймаутов:

```toml
[profile.default]
test-timeout = "20s"
test-threads = 1  # Для сетевых тестов
retries = 1

# Группировка тестов по приоритетам
[[profile.default.overrides]]
filter = 'test(test_node_creation_and_startup) | test(test_peer_information) | test(test_node_diagnostics)'
test-timeout = "10s"

[[profile.default.overrides]]
filter = 'test(test_basic_connection_establishment) | test(test_connection_with_timeout_success) | test(test_disconnect_peer)'
test-timeout = "15s"

[[profile.default.overrides]]
filter = 'test(test_relay_client_basic_scenario) | test(test_relay_client_ping_through_circuit)'
test-timeout = "30s"
```

**Преимущества nextest:**
- Встроенные таймауты на системном уровне
- Умное распараллеливание тестов
- Подробные отчеты и диагностика
- Retry логика для флакки-тестов

## Приоритеты тестирования с таймаутами

### PRIORITY 1: CORE (10 секунд)
- `test_node_creation_and_startup`
- `test_peer_information` 
- `test_node_diagnostics`

### PRIORITY 2: CORE_CONNECTIONS (15 секунд)
- `test_basic_connection_establishment`
- `test_connection_with_timeout_success`
- `test_disconnect_peer`

### PRIORITY 3: XAUTH (15 секунд)
- Аутентификация и безопасность

### PRIORITY 4: XSTREAM (20 секунд)
- Потоковая передача данных

### PRIORITY 5: DISCOVERY (25 секунд)
- DHT поиск
- Bootstrap серверы

### PRIORITY 6: CONNECTIVITY (30 секунд)
- `test_relay_client_basic_scenario`
- `test_relay_client_ping_through_circuit`

## Логирование таймаутов

При срабатывании таймаута тест должен:
1. Выводить понятное сообщение об ошибке
2. Указывать время таймаута
3. Очищать ресурсы перед завершением

## Мониторинг и отладка

### Команды для отладки таймаутов

```bash
# Запуск с детальным выводом
cargo test -- --nocapture --test-threads=1

# Запуск конкретного теста
cargo test test_relay_client_basic_scenario -- --nocapture

# Проверка времени выполнения
time cargo test -- --test-threads=1
```

### Анализ проблемных тестов

Если тест постоянно превышает таймаут:
1. Проверить логику ожидания событий
2. Увеличить таймаут для конкретного теста
3. Добавить промежуточные проверки состояния
4. Убедиться в корректной очистке ресурсов

## Best Practices

1. **Всегда использовать явные таймауты** - не полагаться на дефолтные настройки
2. **Соответствовать приоритетам** - использовать соответствующие времена для каждого типа тестов
3. **Логировать завершение** - выводить сообщения о успешном завершении тестов
4. **Очищать ресурсы** - гарантировать корректную очистку даже при таймауте
5. **Тестировать изолированно** - использовать `--test-threads=1` для сетевых тестов

## Примеры реализации

### Базовый тест с таймаутом

```rust
#[tokio::test]
async fn test_basic_functionality() {
    let test_timeout = Duration::from_secs(10);
    
    let result = tokio::time::timeout(test_timeout, async {
        // Тестовый код
        let (node, commander, _, _) = create_test_node().await?;
        
        // Проверки
        assert!(commander.get_peer_info().await.is_ok());
        
        Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => println!("✅ Test completed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("⏰ Test timed out ({}s)", test_timeout.as_secs()),
    }
}
```

### Сетевой тест с таймаутом

```rust
#[tokio::test]
async fn test_network_connection() {
    let test_timeout = Duration::from_secs(20);
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создание нод
        let (server, client) = create_test_nodes().await?;
        
        // Установление соединения
        client.connect(server.addr()).await?;
        
        // Проверка соединения
        wait_for_connection(&mut client.events(), server.peer_id(), 5).await?;
        
        Ok(())
    }).await;
    
    // Обработка результата...
}
```

## Заключение

Соблюдение политики таймаутов гарантирует:
- Стабильность тестовой среды
- Предсказуемое время выполнения
- Быстрое обнаружение проблем
- Корректную очистку ресурсов

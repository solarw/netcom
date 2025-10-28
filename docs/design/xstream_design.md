# XStream Design Documentation

## Обзор

XStream - это инновационный протокол потоковой передачи данных для NetCom, реализующий концепцию двойных потоков по аналогии с Unix stdout/stderr. Основная идея - разделение основного потока данных и потока ошибок для надежной асинхронной коммуникации.

## Архитектура

### Основные концепции

#### 1. Двойная система потоков
- **Основной поток** - для передачи данных
- **Поток ошибок** - для асинхронных уведомлений об ошибках
- **Независимая обработка** - ошибки не прерывают основной поток

#### 2. Состояния потока
```rust
pub enum StreamState {
    Open,           // Поток активен
    Closed,         // Поток закрыт корректно
    Error(String),  // Поток завершен с ошибкой
}
```

#### 3. XStream структура
```rust
pub struct XStream {
    pub main_stream: Substream,    // Основной поток данных
    pub error_stream: Substream,   // Поток ошибок
    pub state: StreamState,        // Текущее состояние
    pub peer_id: PeerId,           // Удаленный узел
    pub direction: StreamDirection,// Направление (Inbound/Outbound)
}
```

## Процесс работы

### 1. Установление соединения

```
Инициатор (Alice)           Респондер (Bob)
     | --- Open Stream Request ---> |
     | <--- Main Stream Open ------ |
     | <--- Error Stream Open ----- |
     | --- Stream Ready ----------> |
```

### 2. Передача данных

```rust
// Основной поток данных
async fn send_data(stream: &mut XStream, data: &[u8]) -> Result<(), XStreamError> {
    stream.main_stream.write_all(data).await?;
    Ok(())
}

// Асинхронная отправка ошибки
async fn send_error(stream: &mut XStream, error: &str) -> Result<(), XStreamError> {
    stream.error_stream.write_all(error.as_bytes()).await?;
    Ok(())
}
```

### 3. Чтение данных

```rust
// Чтение из основного потока
async fn read_data(stream: &mut XStream) -> Result<Vec<u8>, XStreamError> {
    let mut buffer = Vec::new();
    stream.main_stream.read_to_end(&mut buffer).await?;
    Ok(buffer)
}

// Асинхронное чтение ошибок
async fn read_errors(stream: &mut XStream) -> Result<Vec<String>, XStreamError> {
    let mut errors = Vec::new();
    while let Ok(error_data) = stream.error_stream.try_read() {
        errors.push(String::from_utf8_lossy(&error_data).to_string());
    }
    Ok(errors)
}
```

## Реализация

### Структура модулей

```
protocols/xstream/
├── src/
│   ├── lib.rs              # Основной модуль
│   ├── behaviour.rs        # NetworkBehaviour
│   ├── xstream.rs          # Основная структура XStream
│   ├── handler.rs          # ConnectionHandler
│   ├── protocol.rs         # Протокол передачи
│   ├── events.rs           # События потоков
│   ├── types.rs            # Типы данных
│   ├── xstream_error.rs    # Обработка ошибок
│   ├── xstream_state.rs    # Управление состоянием
│   ├── pending_streams.rs  # Ожидающие потоки
│   ├── header.rs           # Заголовки сообщений
│   └── utils.rs            # Вспомогательные функции
└── tests/
    ├── real_network_swarm_tests.rs
    ├── real_xstream_exchange_tests.rs
    └── connection_handler_*_tests.rs
```

### Key Features

#### 1. Надежность
- **Двойные потоки** - разделение данных и ошибок
- **Асинхронные уведомления** - ошибки не блокируют данные
- **Грациозное восстановление** - обработка сетевых сбоев
- **Таймауты и повторные попытки** - устойчивость к проблемам сети

#### 2. Производительность
- **Неблокирующие операции** - асинхронная обработка
- **Буферизация данных** - эффективное использование сети
- **Минимальные накладные расходы** - оптимизированные заголовки
- **Параллельная обработка** - одновременная работа потоков

#### 3. Гибкость
- **Поддержка различных типов данных** - байты, сообщения, файлы
- **Настраиваемые политики** - таймауты, размеры буферов
- **Расширяемая архитектура** - поддержка новых функций

## Интеграция с NetCom

### Связь с высокоуровневым API

```python
# Python API через PyO3
class XStream:
    async def write(self, data: bytes) -> None:
        """Запись в основной поток"""
    
    async def read(self, size: int = -1) -> bytes:
        """Чтение из основного потока"""
    
    async def write_error(self, error: str) -> None:
        """Отправка ошибки в поток ошибок"""
    
    async def read_error(self) -> Optional[str]:
        """Чтение ошибки из потока ошибок"""
    
    async def close(self) -> None:
        """Корректное закрытие потока"""
```

### Примеры использования

#### Сценарий 1: Передача файла с обработкой ошибок
```python
async def transfer_file_with_error_handling():
    stream = await node.open_stream(peer_id)
    
    try:
        # Основной поток - передача файла
        with open("large_file.dat", "rb") as file:
            while chunk := file.read(8192):
                await stream.write(chunk)
        
        # Поток ошибок - мониторинг проблем
        async def error_monitor():
            while True:
                error = await stream.read_error()
                if error:
                    print(f"Получена ошибка: {error}")
                    # Обработка ошибки без прерывания передачи
        
        # Запуск мониторинга ошибок
        asyncio.create_task(error_monitor())
        
    except Exception as e:
        # Отправка ошибки в поток ошибок
        await stream.write_error(f"Transfer failed: {e}")
    
    finally:
        await stream.close()
```

#### Сценарий 2: Двусторонняя коммуникация
```python
async def bidirectional_chat():
    stream = await node.open_stream(friend_netid)
    
    # Отправка сообщений
    async def send_messages():
        while True:
            message = await get_user_input()
            await stream.write(message.encode())
    
    # Получение сообщений
    async def receive_messages():
        while True:
            data = await stream.read()
            if data:
                print(f"Получено: {data.decode()}")
    
    # Мониторинг ошибок соединения
    async def monitor_errors():
        while True:
            error = await stream.read_error()
            if error:
                print(f"Ошибка соединения: {error}")
    
    # Параллельное выполнение
    await asyncio.gather(
        send_messages(),
        receive_messages(),
        monitor_errors()
    )
```

## Протокол передачи

### Формат сообщений

#### Заголовок сообщения
```rust
pub struct MessageHeader {
    pub stream_type: StreamType,   // MAIN или ERROR
    pub message_id: u64,           // Идентификатор сообщения
    pub total_size: u32,           // Общий размер данных
    pub chunk_index: u32,          // Индекс чанка
    pub total_chunks: u32,         // Всего чанков
    pub flags: u8,                 // Флаги (сжатие, шифрование и т.д.)
}
```

#### Типы потоков
```rust
pub enum StreamType {
    Main = 0x01,    // Основной поток данных
    Error = 0x02,   // Поток ошибок
    Control = 0x03, // Управляющие сообщения
}
```

### Обработка ошибок

#### Иерархия ошибок
```rust
pub enum XStreamError {
    NetworkError(io::Error),           // Сетевые ошибки
    ProtocolError(String),             // Ошибки протокола
    StreamClosed,                      // Поток закрыт
    Timeout,                           // Таймаут операции
    AuthenticationError(String),       // Ошибки аутентификации
    InvalidState(StreamState),         // Неверное состояние
}
```

#### Восстановление после ошибок
- **Автоматические повторные попытки** для временных ошибок
- **Грациозная деградация** при частичных сбоях
- **Уведомление пользователя** через поток ошибок
- **Корректное освобождение ресурсов**

## Безопасность

### Меры защиты

#### 1. Целостность данных
- **Проверка заголовков** - валидация всех полей
- **Контроль размеров** - предотвращение переполнения
- **Верификация состояний** - корректность переходов между состояниями

#### 2. Защита от атак
- **Rate limiting** - ограничение частоты запросов
- **Размеры буферов** - защита от DoS-атак
- **Валидация данных** - проверка входящих сообщений

#### 3. Конфиденциальность
- **Шифрование данных** (планируется)
- **Аутентификация соединений** через XAuth
- **Защита метаданных** - минимальная информация в заголовках

## Производительность

### Оптимизации

#### 1. Сетевые оптимизации
- **Пакетная обработка** - группировка мелких сообщений
- **Компрессия данных** (опционально)
- **Приоритизация трафика** - критичные сообщения вперед

#### 2. Оптимизации памяти
- **Пулы буферов** - переиспользование памяти
- **Ленивая аллокация** - выделение по необходимости
- **Эффективное освобождение** - своевременный возврат памяти

#### 3. Оптимизации CPU
- **Асинхронная обработка** - неблокирующие операции
- **Минимальные копирования** - работа с ссылками
- **Эффективные структуры данных** - оптимизированные контейнеры

## Будущие улучшения

### Планируемые функции

1. **Поддержка QoS** - качество обслуживания для разных типов трафика
2. **Мультиплексирование** - несколько логических потоков в одном физическом
3. **Приоритизация** - управление очередями сообщений
4. **Адаптивное сжатие** - автоматический выбор алгоритма сжатия
5. **End-to-end шифрование** - полная защита данных

### Совместимость

XStream спроектирован для интеграции с:
- Существующими сетевыми протоколами
- Системами мониторинга и логирования
- Инструментами отладки и профилирования
- Стандартами безопасности

## Заключение

XStream предоставляет надежную и эффективную систему потоковой передачи данных для NetCom, реализующую инновационную концепцию двойных потоков. Архитектура обеспечивает высокую производительность, надежность и гибкость, делая XStream идеальным решением для построения современных распределенных приложений.

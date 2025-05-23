# Полное описание системы XStream

## Общий обзор

XStream — это система двойных потоков, предназначенная для надежной передачи данных и обработки ошибок в P2P сетях на базе libp2p. Ключевая особенность заключается в использовании **двух связанных потоков** для каждого логического соединения: основного потока для данных и потока ошибок для передачи сообщений об ошибках.

## Основные концепции

### 1. Двойной поток

Каждый XStream состоит из двух связанных libp2p потоков:
- **Main Stream (основной поток)** — используется для передачи основных данных
- **Error Stream (поток ошибок)** — используется для асинхронного сообщения об ошибках

### 2. Направление потока

Потоки имеют направление, определяющее их поведение:
- **Inbound (входящий)** — поток, полученный от удаленного узла, позволяет писать ошибки
- **Outbound (исходящий)** — поток, созданный локальным узлом, позволяет читать ошибки

### 3. Идентификация потоков

Потоки идентифицируются и сопоставляются с помощью:
- **XStreamID** — уникальный 128-битный идентификатор, общий для обоих потоков пары
- **SubstreamType** — тип подпотока (Main или Error)
- **Заголовок потока** — содержит XStreamID и SubstreamType в формате NetworkEndian

### 4. Жизненный цикл потоков

- **Создание** — оба потока (main и error) должны быть созданы в течение 15 секунд
- **Использование** — чтение/запись данных через основной поток, мониторинг ошибок через поток ошибок
- **Закрытие** — при закрытии одного потока, другой также закрывается
- **Обработка ошибок** — сообщения об ошибках могут быть отправлены через поток ошибок

## Детальная архитектура

### Структура XStream

```rust
pub struct XStream {
    // Основной поток для обмена данными
    pub stream_main_read: Arc<Mutex<ReadHalf<Stream>>>,
    pub stream_main_write: Arc<Mutex<WriteHalf<Stream>>>,
    
    // Поток ошибок
    pub stream_error_read: Arc<Mutex<Option<ReadHalf<Stream>>>>,
    pub stream_error_write: Arc<Mutex<Option<WriteHalf<Stream>>>>,
    
    // Идентификация потока
    pub id: u128,
    pub peer_id: PeerId,
    pub direction: XStreamDirection,
    
    // Уведомления о закрытии
    closure_notifier: Option<mpsc::UnboundedSender<(PeerId, u128)>>,
    
    // Состояние потока
    state: Arc<AtomicU8>,
    
    // Время создания для отслеживания таймаутов
    created_at: Instant,
}
```

### Основные перечисления

```rust
// Направление потока
pub enum XStreamDirection {
    Inbound = 0,  // Входящий поток
    Outbound = 1, // Исходящий поток
}

// Тип подпотока
pub enum SubstreamType {
    Main = 0,  // Основной поток
    Error = 1, // Поток ошибок
}

// Состояния потока
pub enum XStreamState {
    Open = 0,         // Открыт
    LocalClosed = 1,  // Закрыт локально
    RemoteClosed = 2, // Закрыт удаленно
    FullyClosed = 3,  // Полностью закрыт
    Error = 4,        // Произошла ошибка
}
```

### Заголовок потока

```rust
pub struct XStreamHeader {
    pub stream_id: u128,           // Идентификатор потока
    pub stream_type: SubstreamType, // Тип подпотока
}
```

## Процесс создания и использования XStream

### 1. Создание исходящего потока (Outbound)

1. **Запрос на открытие**:
   ```rust
   behaviour.open_stream(peer_id, response_sender)
   ```

2. **Открытие основного потока**:
   - Генерация нового XStreamID
   - Открытие основного потока через libp2p Swarm
   - Запись заголовка в основной поток (XStreamID, SubstreamType::Main)
   - Создание неполного XStream с основным потоком

3. **Открытие потока ошибок**:
   - Открытие второго потока через libp2p Swarm
   - Запись заголовка в поток ошибок (тот же XStreamID, SubstreamType::Error)
   - Добавление потока ошибок к существующему XStream
   - Уведомление о полном создании XStream

4. **Таймаут**:
   - Если поток ошибок не создается в течение 15 секунд, основной поток закрывается и возвращается ошибка

### 2. Обработка входящего потока (Inbound)

1. **Получение первого подпотока**:
   - Чтение заголовка для определения XStreamID и типа подпотока
   - Сохранение подпотока в pending_streams с привязкой к XStreamID

2. **Получение второго подпотока**:
   - Чтение заголовка для определения XStreamID и типа подпотока
   - Поиск в pending_streams подпотока с таким же XStreamID
   - Проверка, что типы подпотоков разные (Main и Error)
   - Создание полного XStream из двух подпотоков
   - Уведомление о новом входящем XStream

3. **Таймаут**:
   - Если второй подпоток не получен в течение 15 секунд, первый подпоток закрывается

### 3. Чтение и запись данных

1. **Запись данных**:
   ```rust
   xstream.write_all(data).await
   ```
   - Проверка состояния потока
   - Запись данных в основной поток
   - Обработка ошибок подключения

2. **Чтение данных**:
   ```rust
   let data = xstream.read().await
   ```
   - Проверка состояния потока
   - Мониторинг потока ошибок на наличие сообщений об ошибках
   - Чтение данных из основного потока
   - Приоритет сообщениям об ошибках над данными

### 4. Обработка ошибок

1. **Запись ошибки** (только для входящих потоков):
   ```rust
   xstream.write_error("Error message").await
   ```
   - Проверка, что поток входящий (Inbound)
   - Запись сообщения об ошибке в поток ошибок
   - Закрытие обоих потоков

2. **Чтение ошибки** (автоматически при чтении основного потока):
   - Мониторинг потока ошибок во время операций чтения
   - Прерывание операции чтения, если получено сообщение об ошибке
   - Возврат ошибки с полученным сообщением

3. **Чтение оставшихся данных после ошибки**:
   ```rust
   let remaining_data = xstream.read_rest_after_error().await
   ```
   - Попытка дочитать данные из основного потока после получения ошибки

### 5. Закрытие потоков

1. **Явное закрытие**:
   ```rust
   xstream.close().await
   ```
   - Для входящих потоков - запись маркера "нет ошибок" перед закрытием
   - Закрытие основного потока
   - Закрытие потока ошибок
   - Уведомление о закрытии XStream

2. **Автоматическое закрытие**:
   - При ошибке одного из потоков - закрытие обоих
   - При EOF основного потока - чтение ошибок, если есть, затем закрытие
   - При таймауте ожидания парного потока - закрытие существующего потока

## Технические детали

### 1. Сериализация заголовков

```rust
// Запись заголовка
pub async fn write_header(stream: &mut WriteHalf<Stream>, header: &XStreamHeader) -> Result<(), io::Error> {
    let mut buffer = Vec::new();
    buffer.write_u128::<NetworkEndian>(header.stream_id)?; // 16 байт
    buffer.write_u8(header.stream_type as u8)?;           // 1 байт
    stream.write_all(&buffer).await?;
    stream.flush().await?;
    Ok(())
}

// Чтение заголовка
pub async fn read_header(stream: &mut ReadHalf<Stream>) -> Result<XStreamHeader, io::Error> {
    let mut id_buf = vec![0u8; 16];
    stream.read_exact(&mut id_buf).await?;
    
    let mut cursor = Cursor::new(id_buf);
    let stream_id = cursor.read_u128::<NetworkEndian>()?;
    
    let mut type_buf = [0u8; 1];
    stream.read_exact(&mut type_buf).await?;
    
    let stream_type = SubstreamType::from(type_buf[0]);
    
    Ok(XStreamHeader { stream_id, stream_type })
}
```

### 2. Правила направления потоков

- **Входящий поток (Inbound)**:
  - Может только писать ошибки через `write_error()`
  - Должен отправлять маркер "нет ошибок" при нормальном закрытии
  - При сбое основного потока, отправляет сообщение об ошибке в поток ошибок

- **Исходящий поток (Outbound)**:
  - Может только читать ошибки
  - При закрытии основного потока, читает любые сообщения об ошибках
  - Генерирует ошибку, если поток ошибок закрывается раньше основного

### 3. Обработка состояний потока

```rust
// Переход состояний
fn set_state(&self, new_state: XStreamState) {
    let current_state = self.state();
    
    // Правила перехода состояний
    let final_state = match (current_state, new_state) {
        // Если уже полностью закрыт или ошибка, остаемся в этом состоянии
        (XStreamState::FullyClosed, _) => XStreamState::FullyClosed,
        (XStreamState::Error, _) => XStreamState::Error,
        
        // Если локально закрыт и удаленно закрывается, переходим в полностью закрытый
        (XStreamState::LocalClosed, XStreamState::RemoteClosed) => XStreamState::FullyClosed,
        
        // Если удаленно закрыт и локально закрывается, переходим в полностью закрытый
        (XStreamState::RemoteClosed, XStreamState::LocalClosed) => XStreamState::FullyClosed,
        
        // В других случаях, используем новое состояние
        (_, new_state) => new_state,
    };
    
    // Обновляем состояние
    self.state.store(final_state as u8, Ordering::Release);
}
```

### 4. Управление таймаутами

```rust
// Очистка ожидающих потоков по таймауту
fn cleanup_pending_streams(&mut self) {
    // Запускаем очистку периодически
    if self.last_cleanup.elapsed() < Duration::from_secs(5) {
        return;
    }
    
    self.last_cleanup = Instant::now();
    
    // Находим потоки, которые ждут слишком долго (15 секунд)
    let timeout = Duration::from_secs(15);
    let now = Instant::now();
    
    let expired_streams: Vec<u128> = self.pending_streams
        .iter()
        .filter(|(_, pending)| now.duration_since(pending.created_at) > timeout)
        .map(|(id, _)| *id)
        .collect();
    
    // Удаляем просроченные потоки и генерируем события ошибок
    for stream_id in expired_streams {
        if let Some(pending) = self.pending_streams.remove(&stream_id) {
            // Логирование и отправка события ошибки
            warn!("Pending stream with id={}, type={:?} timed out", 
                  stream_id, pending.stream_type);
            
            self.outgoing_events.push(XStreamHandlerEvent::StreamError {
                stream_id: Some(stream_id),
                error: format!("Timed out waiting for paired stream"),
            });
        }
    }
}
```

## Примеры использования

### Создание и использование исходящего потока

```rust
// Создание исходящего потока
let (response_tx, response_rx) = oneshot::channel();
behaviour.open_stream(peer_id, response_tx).await;

// Ожидание создания потока
let xstream = response_rx.await?;

// Отправка данных
xstream.write_all(b"Hello, world!").await?;

// Чтение ответа
let response = xstream.read_to_end().await?;
println!("Received: {}", String::from_utf8_lossy(&response));

// Закрытие потока
xstream.close().await?;
```

### Обработка входящего потока

```rust
match event {
    XStreamEvent::IncomingStream { stream } => {
        // Чтение данных из входящего потока
        let data = stream.read_to_end().await?;
        println!("Received data: {}", String::from_utf8_lossy(&data));
        
        // Обработка запроса
        let response = process_request(&data);
        
        // Отправка ответа
        stream.write_all(&response).await?;
        
        // Если произошла ошибка при обработке
        if let Err(e) = process_result {
            // Отправка ошибки через поток ошибок
            stream.write_error(&format!("Error: {}", e)).await?;
        }
        
        // Закрытие потока
        stream.close().await?;
    }
}
```

## Преимущества двойного потока XStream

1. **Надежная обработка ошибок**: 
   - Возможность асинхронно отправлять сообщения об ошибках, даже если основной поток занят
   - Четкое разделение данных и сообщений об ошибках

2. **Улучшенный контроль потока**:
   - Возможность прервать операцию чтения при получении ошибки
   - Возможность продолжить чтение данных даже после получения ошибки

3. **Богатая диагностика**:
   - Подробные сообщения об ошибках, независимые от основного потока данных
   - Возможность передавать структурированные сообщения об ошибках

4. **Надежное закрытие**:
   - Улучшенная семантика закрытия с возможностью различать нормальное закрытие и ошибки
   - Маркер "нет ошибок" для подтверждения успешного закрытия

## Ограничения и особенности

1. **Сложность реализации**:
   - Необходимость отслеживания двух потоков вместо одного
   - Сложное управление состояниями и жизненным циклом

2. **Зависимость от таймаутов**:
   - Необходимость обработки таймаутов при создании парных потоков
   - Возможность "потерянных" потоков при сбоях сети

3. **Согласованное закрытие**:
   - Оба потока должны быть корректно закрыты
   - Необходимость синхронизации закрытия потоков

4. **Направленность потоков**:
   - Разная логика для входящих и исходящих потоков
   - Строгие правила использования потоков ошибок

## Заключение

Система XStream с двойными потоками представляет собой мощный механизм для надежной передачи данных в P2P сетях с расширенной поддержкой обработки ошибок. Этот подход особенно полезен для длительных сетевых операций, где важно иметь возможность асинхронно сообщать об ошибках и поддерживать высокую надежность коммуникации.

Реализация требует внимательного управления состояниями потоков, обработки таймаутов и сопоставления парных потоков, но предоставляет значительные преимущества для сложных распределенных систем.
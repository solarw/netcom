# Продление Proof of Representation (POR) - Спецификация

## Обзор

Этот документ описывает механизм продления сессии Proof of Representation (POR) в системе xauth. Продление позволяет поддерживать аутентифицированные соединения без разрыва при истечении срока действия POR.

## Проблемы текущей реализации

### 1. Статичность POR
- POR создается один раз и не может быть обновлен во время работы
- При истечении срока соединение становится неаутентифицированным
- Нет стандартного механизма для обновления credentials

### 2. Отсутствие мониторинга жизненного цикла
- Приложение не получает уведомлений о скором истечении POR
- Нет возможности проактивно запросить новый POR
- Резкое завершение сессии без возможности восстановления

### 3. Edge Cases из тестов
- **Смена владельца**: Новый POR от другого владельца
- **Частые продления**: Атаки с множественными запросами
- **Конфликтующие времена**: Перекрытие старых и новых POR
- **Сетевые задержки**: Время между запросом и получением
- **Невалидные подписи**: Коррупция данных при передаче

## Архитектура продления

### Новые события

```rust
// В events.rs
#[derive(Debug, Clone)]
pub enum PorAuthEvent {
    // ... существующие события ...
    
    // Уведомление о скором истечении POR
    PorExpiringSoon {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        remaining_seconds: u64,
    },
    
    // Запрос на продление POR от удаленного узла
    PorExtensionRequest {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        new_por: ProofOfRepresentation,
    },
    
    // Ответ на запрос продления
    PorExtensionResponse {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        result: PorExtensionResult,
    },
    
    // Уведомление об истечении POR
    PorExpired {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
    },
    
    // Уведомление об истечении POR в соединении
    ConnectionPorExpired {
        peer_id: PeerId,
        connection_id: ConnectionId,
        address: Multiaddr,
        action: PorExpiredAction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PorExtensionResult {
    Accepted(ProofOfRepresentation),
    Rejected(String),
}

#[derive(Debug, Clone)]
pub enum PorExpiredAction {
    Disconnect,      // Разорвать соединение
    WaitForRenewal,  // Ждать нового POR
    GracePeriod(u64), // Период ожидания в секундах
}
```

### Новые структуры данных

```rust
// В definitions.rs
pub struct PorExtensionConfig {
    pub enable_proactive_renewal: bool,
    pub renewal_threshold_seconds: u64, // За сколько секунд до истечения начинать продление
    pub max_renewal_attempts: u32,
    pub renewal_timeout: Duration,
    pub grace_period: Duration,
}

pub struct PorRenewalState {
    pub original_owner_public_key: PublicKey,
    pub renewal_attempts: u32,
    pub last_renewal_request: Option<Instant>,
    pub pending_renewal: bool,
}
```

## Механизмы продления

### 1. Проактивное продление

**Триггер**: За N секунд до истечения POR
**Действие**: Автоматический запрос нового POR

```rust
impl PorAuthBehaviour {
    fn check_pending_renewals(&mut self) {
        for (conn_id, conn_data) in &self.connections {
            if let Some(por) = self.get_current_por() {
                if let Ok(Some(remaining)) = por.remaining_time() {
                    if remaining <= self.config.renewal_threshold_seconds {
                        self.trigger_proactive_renewal(*conn_id);
                    }
                }
            }
        }
    }
    
    fn trigger_proactive_renewal(&mut self, connection_id: ConnectionId) {
        // Уведомляем приложение о необходимости нового POR
        if let Some(conn) = self.connections.get(&connection_id) {
            self.pending_events.push_back(ToSwarm::GenerateEvent(
                PorAuthEvent::PorExpiringSoon {
                    peer_id: conn.peer_id,
                    connection_id,
                    address: conn.address.clone(),
                    remaining_seconds: remaining,
                }
            ));
        }
    }
}
```

### 2. Реактивное продление

**Триггер**: Получение `PorExpired` уведомления
**Действие**: Запрос нового POR с сохранением состояния

```rust
impl PorAuthBehaviour {
    fn handle_por_expired(&mut self, connection_id: ConnectionId) {
        if let Some(conn) = self.connections.get(&connection_id) {
            // Сохраняем состояние для возможного восстановления
            let renewal_state = PorRenewalState {
                original_owner_public_key: self.por.owner_public_key.clone(),
                renewal_attempts: 0,
                last_renewal_request: Some(Instant::now()),
                pending_renewal: true,
            };
            
            self.pending_renewals.insert(connection_id, renewal_state);
            
            // Уведомляем приложение
            self.pending_events.push_back(ToSwarm::GenerateEvent(
                PorAuthEvent::ConnectionPorExpired {
                    peer_id: conn.peer_id,
                    connection_id,
                    address: conn.address.clone(),
                    action: PorExpiredAction::WaitForRenewal,
                }
            ));
        }
    }
}
```

### 3. Обработка запросов продления

```rust
impl PorAuthBehaviour {
    fn handle_por_extension_request(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        new_por: ProofOfRepresentation,
    ) {
        // Проверяем безопасность
        let validation_result = self.validate_por_extension(&new_por);
        
        match validation_result {
            Ok(()) => {
                // Принимаем новый POR
                self.update_por(new_por.clone());
                
                self.pending_events.push_back(ToSwarm::GenerateEvent(
                    PorAuthEvent::PorExtensionResponse {
                        peer_id,
                        connection_id,
                        address: self.get_connection_address(connection_id),
                        result: PorExtensionResult::Accepted(new_por),
                    }
                ));
            }
            Err(reason) => {
                // Отклоняем запрос
                self.pending_events.push_back(ToSwarm::GenerateEvent(
                    PorAuthEvent::PorExtensionResponse {
                        peer_id,
                        connection_id,
                        address: self.get_connection_address(connection_id),
                        result: PorExtensionResult::Rejected(reason),
                    }
                ));
            }
        }
    }
    
    fn validate_por_extension(&self, new_por: &ProofOfRepresentation) -> Result<(), String> {
        // 1. Проверяем, что тот же владелец
        if new_por.owner_public_key != self.por.owner_public_key {
            return Err("Different owner public key".to_string());
        }
        
        // 2. Проверяем валидность нового POR
        new_por.validate()?;
        
        // 3. Проверяем, что новый POR не истек
        if new_por.is_expired()? {
            return Err("New POR has already expired".to_string());
        }
        
        // 4. Проверяем, что время выпуска логичное
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("System time error: {}", e))?
            .as_secs();
            
        if new_por.issued_at > now + 300 { // Не более 5 минут в будущем
            return Err("New POR issued too far in the future".to_string());
        }
        
        Ok(())
    }
}
```

## Безопасность

### 1. Проверка владельца
```rust
fn verify_same_owner(&self, new_por: &ProofOfRepresentation) -> bool {
    new_por.owner_public_key == self.por.owner_public_key
}
```

### 2. Ограничение попыток продления
```rust
const MAX_RENEWAL_ATTEMPTS: u32 = 3;
const RENEWAL_COOLDOWN: Duration = Duration::from_secs(60);

fn can_request_renewal(&self, connection_id: ConnectionId) -> bool {
    if let Some(state) = self.pending_renewals.get(&connection_id) {
        if state.renewal_attempts >= MAX_RENEWAL_ATTEMPTS {
            return false;
        }
        
        if let Some(last_request) = state.last_renewal_request {
            if last_request.elapsed() < RENEWAL_COOLDOWN {
                return false;
            }
        }
    }
    true
}
```

### 3. Защита от replay-атак
```rust
fn is_replay_attack(&self, new_por: &ProofOfRepresentation) -> bool {
    // Проверяем, что время выпуска нового POR больше старого
    new_por.issued_at <= self.por.issued_at
}
```

## API для приложения

### Конфигурация
```rust
let config = PorExtensionConfig {
    enable_proactive_renewal: true,
    renewal_threshold_seconds: 30, // За 30 секунд до истечения
    max_renewal_attempts: 3,
    renewal_timeout: Duration::from_secs(10),
    grace_period: Duration::from_secs(60), // 1 минута на восстановление
};

let auth_behaviour = PorAuthBehaviour::new(por)
    .with_extension_config(config);
```

### Обработка событий
```rust
match event {
    PorAuthEvent::PorExpiringSoon { peer_id, connection_id, remaining_seconds, .. } => {
        // Запросить новый POR у владельца
        let new_por = request_new_por_from_owner();
        auth_behaviour.submit_por_renewal(connection_id, new_por);
    }
    
    PorAuthEvent::ConnectionPorExpired { peer_id, connection_id, action, .. } => {
        match action {
            PorExpiredAction::WaitForRenewal => {
                // Попытаться получить новый POR
                attempt_por_renewal(peer_id);
            }
            PorExpiredAction::Disconnect => {
                // Разорвать соединение
                swarm.disconnect_peer_id(peer_id);
            }
            PorExpiredAction::GracePeriod(seconds) => {
                // Ждать указанное время
                schedule_renewal_attempt(peer_id, seconds);
            }
        }
    }
    
    PorAuthEvent::PorExtensionRequest { peer_id, new_por, .. } => {
        // Решить, принимать ли запрос продления
        if should_accept_extension(peer_id, &new_por) {
            auth_behaviour.accept_por_extension(peer_id, new_por);
        } else {
            auth_behaviour.reject_por_extension(peer_id, "Policy violation");
        }
    }
}
```

## Сценарии использования

### 1. Успешное проактивное продление
```
Время: T-30 секунд до истечения
Событие: PorExpiringSoon
Действие: Приложение запрашивает новый POR
Результат: Прозрачная замена, соединение продолжает работу
```

### 2. Реактивное продление после истечения
```
Время: T+0 (истечение)
Событие: ConnectionPorExpired с WaitForRenewal
Действие: Приложение пытается получить новый POR
Результат: Восстановление или graceful degradation
```

### 3. Отказ в продлении
```
Событие: PorExtensionRequest
Проверка: Новый POR от другого владельца
Действие: PorExtensionResponse с Rejected
Результат: Соединение разрывается
```

## Миграция и обратная совместимость

### Для узлов без поддержки продления
- Используется стандартный механизм аутентификации
- При истечении POR соединение разрывается
- Нет уведомлений о продлении

### Для узлов с поддержкой продления
- Автоматическое определение возможностей через metadata
- Fallback на стандартное поведение при несовместимости
- Постепенное внедрение без breaking changes

## Тестирование

### Unit тесты
```rust
#[test]
fn test_proactive_renewal_trigger() {
    // Тест уведомления за 30 секунд до истечения
}

#[test]
fn test_por_extension_validation() {
    // Тест проверки безопасности продления
}

#[test]
fn test_renewal_attempt_limiting() {
    // Тест ограничения попыток продления
}
```

### Интеграционные тесты
```rust
#[test]
fn test_seamless_por_renewal() {
    // Тест прозрачного продления во время работы
}

#[test]
fn test_graceful_degradation_on_renewal_failure() {
    // Тест graceful degradation при неудачном продлении
}
```

## Заключение

Механизм продления POR обеспечивает:
- **Непрерывность сессии** без разрыва соединений
- **Безопасность** через строгие проверки владельца
- **Гибкость** через настраиваемые политики
- **Обратную совместимость** с существующими узлами

Реализация этого механизма значительно улучшает пользовательский опыт и надежность системы аутентификации.

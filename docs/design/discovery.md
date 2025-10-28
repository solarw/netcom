# Discovery Design Documentation

## Обзор

Discovery - это критически важный компонент NetCom, обеспечивающий механизмы обнаружения узлов и сущностей в децентрализованной сети. Основная цель - предоставить надежные способы поиска и подключения к узлам без централизованных серверов.

## Основные цели Discovery

### 1. Поиск узлов по NetID
- **Найти все PeerId**, принадлежащие определенному NetID
- **Обнаружить активные узлы** сущности в сети
- **Поддержка multi-node representation** - одна сущность, множество узлов

### 2. Поиск адресов подключения
- **Найти все Multiaddr** для конкретного PeerId
- **Обнаружить доступные пути** подключения через NAT
- **Определить оптимальные маршруты** для соединения

### 3. Локальное и nearby обнаружение
- **Обнаружение в локальной сети** - узлы в той же подсети
- **Nearby обнаружение** - географически близкие узлы
- **Сетевые proximity** - узлы с минимальной задержкой

## Архитектура Discovery

### Основные компоненты

#### 1. Discovery Service
```rust
pub struct DiscoveryService {
    kad_behaviour: KadBehaviour,      // Kademlia DHT
    mdns_behaviour: MdnsBehaviour,   // mDNS для локальной сети
    custom_discovery: CustomDiscovery, // Специализированные сервисы
    discovery_cache: DiscoveryCache,  // Кэш результатов
}
```

#### 2. Discovery Query
```rust
pub enum DiscoveryQuery {
    FindNodesByNetId(NetID),          // Поиск узлов по NetID
    FindAddressesByPeerId(PeerId),    // Поиск адресов по PeerId
    FindLocalNodes,                   // Поиск узлов в локальной сети
    FindNearbyNodes(GeoLocation),     // Поиск nearby узлов
}
```

#### 3. Discovery Result
```rust
pub struct DiscoveryResult {
    pub nodes: HashMap<PeerId, NodeInfo>, // Найденные узлы
    pub addresses: HashMap<PeerId, Vec<Multiaddr>>, // Адреса подключения
    pub source: DiscoverySource,      // Источник обнаружения
    pub timestamp: u64,               // Время обнаружения
}
```

## Механизмы обнаружения

### 1. Kademlia DHT (Distributed Hash Table)

#### Принцип работы
- **Распределенное хранение** информации об узлах
- **Эффективный поиск** через bucket-based routing
- **Автоматическая репликация** данных в сети

#### Использование для NetID
```rust
// Хранение в DHT: NetID -> список PeerId
let key = kad::RecordKey::new(&netid.to_bytes());
let record = kad::Record {
    key,
    value: peer_list.to_bytes(),
    publisher: None,
    expires: Some(Instant::now() + Duration::from_hours(24)),
};
```

#### Преимущества
- **Децентрализованность** - нет единой точки отказа
- **Масштабируемость** - работает в больших сетях
- **Устойчивость** - автоматическое восстановление

#### Проблемы
- **Задержка поиска** - требуется несколько запросов
- **Актуальность данных** - информация может устаревать
- **Bootstrap зависимость** - нужны начальные узлы

### 2. mDNS (Multicast DNS)

#### Принцип работы
- **Multicast рассылка** в локальной сети
- **Автоматическое обнаружение** nearby узлов
- **Нулевая конфигурация** - работает из коробки

#### Использование для локального обнаружения
```rust
// Объявление сервиса в локальной сети
mdns::Service::new("_netcom._tcp", netid.to_string())
    .with_addresses(listen_addresses)
    .announce()?;
```

#### Преимущества
- **Мгновенное обнаружение** в локальной сети
- **Простота использования** - минимальная настройка
- **Энергоэффективность** - для мобильных устройств

#### Проблемы
- **Ограниченный радиус** - только локальная сеть
- **Безопасность** - уязвимость к spoofing-атакам
- **Масштабируемость** - не подходит для глобальных сетей

### 3. Специализированные Discovery сервисы

#### API-based Discovery
```rust
// Запрос к специализированному сервису
async fn query_discovery_service(netid: &NetID) -> Result<Vec<PeerInfo>> {
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("https://discovery.netcom.io/nodes/{}", netid))
        .send()
        .await?;
    
    response.json().await
}
```

#### Преимущества
- **Высокая скорость** - прямой запрос к сервису
- **Актуальность данных** - централизованное обновление
- **Дополнительные метрики** - нагрузка, геолокация, доступность

#### Проблемы
- **Централизация** - зависимость от сервиса
- **Single point of failure** - отказ сервиса блокирует discovery
- **Конфиденциальность** - раскрытие информации третьим лицам

### 4. Blockchain-based Discovery

#### Принцип работы
- **Регистрация узлов** в смарт-контракте
- **Децентрализованное хранение** - immutable ledger
- **Криптографическая верификация** - доказательство принадлежности

#### Использование
```solidity
// Смарт-контракт для регистрации узлов
contract NetComRegistry {
    mapping(NetID => NodeInfo[]) public nodes;
    
    function registerNode(NetID netid, PeerId peerid, Multiaddr[] memory addresses) public {
        // Регистрация с проверкой подписи
        require(verifySignature(netid, msg.sender), "Invalid signature");
        nodes[netid].push(NodeInfo(peerid, addresses, block.timestamp));
    }
}
```

#### Преимущества
- **Неизменяемость** - данные нельзя изменить
- **Децентрализация** - нет центрального контроля
- **Доверие** - криптографические гарантии

#### Проблемы
- **Высокая стоимость** - gas fees для операций
- **Задержки** - время подтверждения транзакций
- **Сложность** - интеграция с блокчейн-сетями

### 5. DNS-based Discovery

#### Принцип работы
- **DNS TXT записи** для хранения информации
- **SRV записи** для сервисного обнаружения
- **DNS-over-HTTPS** для безопасности

#### Использование
```rust
// Запрос DNS TXT записей
async fn dns_discovery(domain: &str) -> Result<Vec<Multiaddr>> {
    let resolver = trust_dns_resolver::AsyncResolver::tokio();
    let response = resolver.txt_lookup(&format!("_netcom.{}", domain)).await?;
    
    response.into_iter()
        .flat_map(|record| parse_multiaddrs(&record.to_string()))
        .collect()
}
```

#### Преимущества
- **Стандартизация** - использование существующей инфраструктуры
- **Надежность** - DNS как основа интернета
- **Кэширование** - встроенные механизмы кэширования

#### Проблемы
- **Централизация** - зависимость от DNS-серверов
- **Обновления** - задержки распространения DNS-записей
- **Безопасность** - уязвимость к DNS spoofing

## Интеграция с NetID

### Хранение информации в DHT

#### NetID to PeerId mapping
```
Key: netid_<NetID_bytes>
Value: [PeerId1, PeerId2, ...]  # Список узлов сущности
TTL: 24 часа
```

#### PeerId to Address mapping
```
Key: peer_<PeerId_bytes>  
Value: [Multiaddr1, Multiaddr2, ...]  # Адреса подключения
TTL: 6 часов
```

### Процесс обнаружения сущности

```rust
async fn discover_entity_nodes(netid: NetID) -> Result<HashMap<PeerId, NodeInfo>> {
    let mut results = HashMap::new();
    
    // 1. Поиск в DHT
    if let Ok(peer_ids) = kad_discovery.find_nodes_by_netid(&netid).await {
        for peer_id in peer_ids {
            // 2. Поиск адресов для каждого PeerId
            if let Ok(addresses) = kad_discovery.find_addresses_by_peerid(&peer_id).await {
                results.insert(peer_id, NodeInfo { addresses, source: DiscoverySource::Kad });
            }
        }
    }
    
    // 3. Поиск через mDNS (локальная сеть)
    if let Ok(local_nodes) = mdns_discovery.find_local_nodes().await {
        for (peer_id, info) in local_nodes {
            if info.netid == netid {
                results.entry(peer_id).or_insert(info).source = DiscoverySource::Mdns;
            }
        }
    }
    
    // 4. Поиск через специализированные сервисы
    if let Ok(service_nodes) = custom_discovery.query_service(&netid).await {
        results.extend(service_nodes);
    }
    
    Ok(results)
}
```

## Проблемы и решения

### 1. Проблема: Актуальность информации
- **Решение**: TTL-based обновление, heartbeat механизмы
- **Решение**: Multiple source verification
- **Решение**: Probabilistic expiration

### 2. Проблема: Сетевые ограничения
- **Решение**: NAT traversal techniques (STUN, TURN, ICE)
- **Решение**: Relay servers для сложных сетевых условий
- **Решение**: Hybrid approaches (DHT + centralized fallback)

### 3. Проблема: Безопасность
- **Решение**: Cryptographic verification через XAuth
- **Решение**: Rate limiting и anti-spam меры
- **Решение**: Privacy-preserving discovery

### 4. Проблема: Производительность
- **Решение**: Intelligent caching стратегии
- **Решение**: Parallel query execution
- **Решение**: Adaptive timeout management

## Best Practices

### 1. Многоуровневое обнаружение
```rust
// Приоритеты источников discovery
enum DiscoveryPriority {
    LocalNetwork,    // mDNS - самый быстрый
    NearbyNodes,     // Географически близкие
    DHT,            // Глобальная сеть
    FallbackServices // Централизованные сервисы
}
```

### 2. Интеллектуальное кэширование
- **Кэш успешных запросов** - уменьшение нагрузки на сеть
- **Negative caching** - запоминание неудачных попыток
- **Adaptive TTL** - динамическое время жизни кэша

### 3. Обработка ошибок
- **Graceful degradation** - переход на альтернативные методы
- **Exponential backoff** - для повторных запросов
- **Circuit breaker** - предотвращение cascade failures

## Будущие улучшения

### 1. Machine Learning оптимизации
- **Predictive discovery** - предсказание доступности узлов
- **Network topology learning** - оптимизация маршрутов
- **Anomaly detection** - обнаружение подозрительной активности

### 2. Advanced механизмы
- **Federated discovery** - федеративные сервисы
- **Quantum-resistant cryptography** - защита от квантовых атак
- **Zero-knowledge proofs** - privacy-preserving discovery

### 3. Интеграция с Web3
- **IPFS integration** - использование существующей инфраструктуры
- **Blockchain oracles** - доступ к внешним данным
- **DeFi discovery** - интеграция с децентрализованными финансами

## Заключение

Discovery является фундаментальным компонентом NetCom, обеспечивающим возможность децентрализованного обнаружения узлов и сущностей в сети. Многоуровневый подход, сочетающий DHT, mDNS, специализированные сервисы и blockchain-технологии, обеспечивает надежность, производительность и безопасность механизмов обнаружения.

Архитектура спроектирована для масштабирования и адаптации к различным сетевым условиям, что делает NetCom универсальным решением для построения современных распределенных приложений.

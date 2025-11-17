# План создания тестовой среды NAT traversal для xnetwork2

## Цель задачи
Создать тестовую среду для проверки NAT traversal через relay сервер с использованием xnetwork2.

## Архитектура
```
           nat1                nat2
   +----------------+    +----------------+
   |     node1      |    |     node2      |
   +----------------+    +----------------+
           |                    |
           |                    |
           +-------+    +-------+
                   |    |
                +----------------+
                |     relay      |
                +----------------+
                        |
                    public_net
```

## Условия успеха задачи
✅ **Основное условие:** Node1 успешно подключается к Node2 через relay сервер в условиях сетевой изоляции

### Конкретные проверки успеха:
1. **Сетевая изоляция подтверждена:**
   - Node1 и Node2 не могут общаться напрямую (`ping node2` из node1 должен завершиться ошибкой)
   - Каждый узел находится только в своей NAT сети

2. **Relay сервер работает:**
   - Relay слушает на порту 15003
   - Relay доступен из обеих NAT сетей
   - Kademlia DHT работает на relay

3. **Node2 готов к подключениям:**
   - Node2 успешно подключился к relay
   - Node2 получил relay адрес через `setup_listening_node_with_addr`
   - Node2 опубликовал свой peer_id в Kademlia DHT

4. **Node1 находит и подключается к Node2:**
   - Node1 успешно подключился к relay
   - Node1 нашел адреса Node2 через Kademlia DHT
   - Node1 установил соединение с Node2 через relay адрес
   - Оба узла получили события `ConnectionEstablished`

5. **Соединение работает через relay:**
   - Соединение установлено через relay сервер, а не напрямую
   - Данные передаются через relay

## Шаги реализации

### 1. Создание структуры проекта
```
nat-traversal-test/
├── plan.md (этот файл)
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── .env
├── src/
│   ├── relay.rs
│   ├── node.rs
│   └── utils.rs (копия утилит из xnetwork2/tests/utils.rs)
└── scripts/
    ├── entrypoint-relay.sh
    └── entrypoint-node.sh
```

### 2. Создание программы relay
**Конфигурация:** `NodeBuilder::new().with_relay_server().with_kademlia().with_identify()`

**Логика:**
- Загрузка ключа из `NODE_KEY` переменной окружения
- Прослушивание на `/ip4/0.0.0.0/udp/15003/quic-v1` с ожиданием `NewListenAddr`
- Bootstrap Kademlia DHT
- Бесконечный цикл обработки событий

### 3. Создание программы node
**Параметры командной строки:**
- `--relay-address` (обязательный) - адрес relay сервера
- `--relay-peer-id` (обязательный) - peer_id relay сервера
- `--target-peer` (опциональный) - peer_id узла для подключения

**Логика:**
- Загрузка ключа из `NODE_KEY` переменной окружения
- Подключение к relay с повторными попытками и ожиданием `ConnectionEstablished`
- Получение relay адреса через `setup_listening_node_with_addr` с ожиданием `NewListenAddr`
- Публикация в Kademlia DHT
- Если указан `--target-peer`: поиск адресов в Kademlia и подключение через relay

### 4. Копирование утилит из tests/utils.rs
Скопировать:
- `wait_for_event`
- `setup_listening_node`
- `setup_listening_node_with_addr`
- `dial_and_wait_connection`

### 5. Настройка Cargo.toml
Добавить 2 [[bin]] секции для relay и node

### 6. Создание .env файла
```env
RELAY_KEY=base64_key_here
NODE1_KEY=base64_key_here  
NODE2_KEY=base64_key_here
NODE2_PEER_ID=12D3KooW...
RELAY_PEER_ID=12D3KooW...
RELAY_ADDRESS=172.20.0.10:15003
```

### 7. Создание Dockerfile
Один образ с двумя бинарниками relay и node

### 8. Создание docker-compose.yml
```yaml
version: "3.9"

services:
  relay:
    build: .
    container_name: relay
    command: ["relay"]
    networks:
      - public_net
      - nat1
      - nat2
    environment:
      - NODE_KEY=${RELAY_KEY}
      - RELAY_ADDRESS=${RELAY_ADDRESS}

  node2:
    build: .
    container_name: node2
    command: ["node", "--relay-address", "${RELAY_ADDRESS}", "--relay-peer-id", "${RELAY_PEER_ID}"]
    networks:
      - nat2
    environment:
      - NODE_KEY=${NODE2_KEY}
    depends_on:
      - relay

  node1:
    build: .
    container_name: node1
    command: ["node", "--relay-address", "${RELAY_ADDRESS}", "--relay-peer-id", "${RELAY_PEER_ID}", "--target-peer", "${NODE2_PEER_ID}"]
    networks:
      - nat1
    environment:
      - NODE_KEY=${NODE1_KEY}
    depends_on:
      - relay
      - node2

networks:
  public_net:
    driver: bridge

  nat1:
    driver: bridge
    internal: true

  nat2:
    driver: bridge
    internal: true
```

### 9. Запуск и тестирование
1. `docker-compose up -d`
2. Проверить логи: `docker-compose logs -f`
3. Проверить изоляцию: `docker exec node1 ping node2` (должен завершиться ошибкой)
4. Проверить подключение через relay (логи должны показывать успешное подключение)

## Критерии проверки в логах
- Логи relay показывают обработку подключений от обоих узлов
- Логи node1 показывают успешный поиск node2 в DHT
- Логи node2 показывают получение входящего подключения через relay
- Все узлы показывают события `ConnectionEstablished`

## Финальная проверка
После запуска `docker-compose up` в течение 30 секунд:
- Все 3 контейнера работают
- В логах видно успешное подключение node1 → node2 через relay
- Сетевая изоляция подтверждена тестами ping

**Задача считается успешно выполненной когда все вышеперечисленные условия выполнены.**

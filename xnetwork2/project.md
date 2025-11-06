# XNetwork2 - P2P сеть на базе command-swarm

## Описание проекта
XNetwork2 - это переработанная версия xnetwork, использующая архитектуру command-swarm для управления libp2p swarm. Проект предоставляет структурированный подход к обработке команд и событий через отдельные handlers для каждого behaviour.

## Архитектура

### Основные компоненты:

1. **Behaviour Handlers** - отдельные обработчики для каждого protocol behaviour:
   - `IdentifyHandler` - для libp2p identify protocol
   - `PingHandler` - для libp2p ping protocol  
   - `XAuthHandler` - адаптер для xauth authentication
   - `XStreamHandler` - адаптер для xstream data streaming

2. **Swarm Handler** - обработчик swarm-level операций:
   - `XNetworkSwarmHandler` - управление соединениями, прослушиванием

3. **Main Behaviour** - объединенный behaviour через макрос:
   - `make_command_swarm!` макрос генерирует объединенную структуру

4. **Node Management** - создание и управление SwarmLoop:
   - `Node` - создание и запуск SwarmLoop
   - `Commander` - API для отправки команд

### Ключевые особенности:

- **Модульная архитектура** - каждый behaviour имеет свой handler
- **Типобезопасность** - строгая типизация команд и событий
- **Graceful shutdown** - корректное завершение через SwarmLoopStopper
- **Обратная совместимость** - использование существующих xauth/xstream behaviours

## Структура проекта

```
xnetwork2/
├── src/
│   ├── lib.rs                    # Основной модуль
│   ├── behaviours/               # Behaviour handlers
│   │   ├── mod.rs               # Экспорт всех handlers
│   │   ├── identify/            # Identify handler
│   │   ├── ping/               # Ping handler
│   │   ├── xauth/              # XAuth adapter
│   │   └── xstream/            # XStream adapter
│   ├── swarm_commands.rs        # Swarm-level команды
│   ├── swarm_handler.rs         # Swarm-level обработчик
│   ├── main_behaviour.rs        # Макрос объединения
│   ├── node.rs                 # Создание SwarmLoop
│   └── commander.rs            # API для команд
└── examples/
    └── basic_node.rs           # Пример использования
```

## Принципы реализации

1. **Инкрементальная разработка** - каждый компонент компилируется отдельно
2. **Использование существующих событий** - не переопределяем Events из behaviours
3. **Command-based подход** - все операции через команды
4. **Event-driven архитектура** - обработка событий через handlers

## Зависимости

- **command-swarm** - основа архитектуры
- **libp2p 0.56** - базовые protocols (identify, ping)
- **xauth** - аутентификация
- **xstream** - потоковая передача данных
- **tokio** - асинхронность
- **tracing** - логирование

## План разработки

1. ✅ Базовая структура проекта
2. ✅ Настройка зависимостей
3. ⏳ Создание behaviour handlers (по одному)
4. ⏳ Swarm handler и команды
5. ⏳ Main behaviour с макросом
6. ⏳ Node и Commander
7. ⏳ Примеры и тесты

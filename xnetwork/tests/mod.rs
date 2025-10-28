// tests/mod.rs - Модули тестов для xnetwork

// Общие утилиты для тестирования
pub mod common;

// Приоритет 1: Базовые компоненты ноды
pub mod node_core;

// Приоритет 2: Прямые соединения между нодами
pub mod core_connections;

// Приоритет 3: Аутентификация и безопасность
pub mod xauth;

// Приоритет 4: Потоковая передача данных
pub mod xstream;

// Приоритет 5: Механизмы обнаружения нод
pub mod discovery;

// Приоритет 6: Расширенная сетевая функциональность
pub mod connectivity;

// Bootstrap тесты
pub mod bootstrap;

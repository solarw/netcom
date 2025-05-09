
use libp2p::{Multiaddr, PeerId};
use std::time::Duration;

/// Настройки опорного сервера
#[derive(Clone, Debug)]
pub struct BootstrapServerConfig {
    /// Интервал синхронизации с другими опорными серверами
    pub sync_interval: Duration,
    
    /// Максимальное количество сохраняемых маршрутов
    pub max_routes: usize,
    
    /// Время жизни маршрута в кэше
    pub route_ttl: Duration,
    
    /// Другие известные опорные серверы
    pub bootstrap_nodes: Vec<(PeerId, Vec<Multiaddr>)>,
    
    /// Включить агрессивное объявление себя как опорного сервера
    pub aggressive_announce: bool,
    
    /// Включить расширенное кэширование маршрутов
    pub extended_routing: bool,
}

impl Default for BootstrapServerConfig {
    fn default() -> Self {
        Self {
            sync_interval: Duration::from_secs(300), // 5 минут
            max_routes: 10000,
            route_ttl: Duration::from_secs(3600 * 24), // 24 часа
            bootstrap_nodes: Vec::new(),
            aggressive_announce: true,
            extended_routing: true,
        }
    }
}

/// Статистика работы опорного сервера
#[derive(Default, Debug, Clone)]
pub struct BootstrapServerStats {
    /// Количество запросов на поиск маршрутов
    pub route_requests: usize,
    
    /// Количество успешных ответов на запросы
    pub successful_responses: usize,
    
    /// Количество узлов в расширенном кэше
    pub cached_routes_count: usize,
    
    /// Количество синхронизаций с другими опорными серверами
    pub sync_count: usize,
    
    /// Количество проанонсированных маршрутов
    pub announced_routes: usize,
}
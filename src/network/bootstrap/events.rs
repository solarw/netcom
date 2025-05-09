use super::config::BootstrapServerStats;
use libp2p::{Multiaddr, PeerId};

/// События опорного сервера
#[derive(Debug, Clone)]
pub enum BootstrapEvent {
    /// Опорный сервер активирован
    Activated,
    
    /// Опорный сервер деактивирован
    Deactivated,
    
    /// Синхронизация с другими опорными серверами завершена
    Synced {
        stats: BootstrapServerStats,
    },
    
    /// Добавлен новый маршрут
    RouteAdded {
        peer_id: PeerId,
        addr: Multiaddr,
    },
    
    /// Обновлены маршруты для узла
    RoutingUpdated {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
}
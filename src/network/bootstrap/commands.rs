use super::config::BootstrapServerStats;
use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;
use std::error::Error;
use tokio::sync::oneshot;

/// Команды для опорного сервера
#[derive(Debug)]
pub enum BootstrapCommand {
    /// Активировать опорный сервер
    Activate {
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    /// Деактивировать опорный сервер
    Deactivate {
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    /// Получить статистику опорного сервера
    GetStats {
        response: oneshot::Sender<BootstrapServerStats>,
    },
    
    /// Добавить опорный сервер в список известных
    AddNode {
        peer_id: PeerId,
        addrs: Vec<Multiaddr>,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    /// Удалить опорный сервер из списка известных
    RemoveNode {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
    
    /// Получить список известных опорных серверов
    GetNodes {
        response: oneshot::Sender<HashMap<PeerId, Vec<Multiaddr>>>,
    },
    
    /// Принудительная синхронизация с опорными серверами
    ForceSync {
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
}
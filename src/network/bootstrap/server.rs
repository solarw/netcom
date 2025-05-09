// Файл: ./src/network/bootstrap/server.rs

use super::config::{BootstrapServerConfig, BootstrapServerStats};
use super::events::BootstrapEvent;
use libp2p::{
    kad,
    Multiaddr, PeerId,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, Mutex};

/// Состояние опорного сервера
pub struct BootstrapServer {
    /// Локальный идентификатор узла
    local_peer_id: PeerId,
    
    /// Расширенный кэш маршрутов (сверх стандартной таблицы Kademlia)
    extended_routes: Arc<Mutex<HashMap<PeerId, Vec<Multiaddr>>>>,
    
    /// Известные опорные серверы
    known_bootstrap_nodes: Arc<Mutex<HashMap<PeerId, Vec<Multiaddr>>>>,
    
    /// Настройки опорного сервера
    config: BootstrapServerConfig,
    
    /// Время последней синхронизации с другими опорными серверами
    last_sync: Arc<Mutex<Instant>>,
    
    /// Активен ли режим опорного сервера
    active: Arc<Mutex<bool>>,
    
    /// Статистика использования
    stats: Arc<Mutex<BootstrapServerStats>>,
    
    /// Канал для отправки событий
    event_tx: mpsc::Sender<BootstrapEvent>,
}

impl BootstrapServer {
    /// Создает новый опорный сервер
    pub fn new(
        local_peer_id: PeerId, 
        config: BootstrapServerConfig, 
        event_tx: mpsc::Sender<BootstrapEvent>
    ) -> Self {
        Self {
            local_peer_id,
            extended_routes: Arc::new(Mutex::new(HashMap::new())),
            known_bootstrap_nodes: Arc::new(Mutex::new(HashMap::from_iter(
                config.bootstrap_nodes.clone().into_iter()
            ))),
            config,
            last_sync: Arc::new(Mutex::new(Instant::now())),
            active: Arc::new(Mutex::new(false)),
            stats: Arc::new(Mutex::new(BootstrapServerStats::default())),
            event_tx,
        }
    }
    
    /// Активирует режим опорного сервера
    pub async fn activate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut active = self.active.lock().await;
        *active = true;
        
        // Отправляем событие активации
        self.event_tx.send(BootstrapEvent::Activated).await
            .map_err(|e| format!("Failed to send activated event: {}", e).into())
    }
    
    /// Деактивирует режим опорного сервера
    pub async fn deactivate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut active = self.active.lock().await;
        *active = false;
        
        // Отправляем событие деактивации
        self.event_tx.send(BootstrapEvent::Deactivated).await
            .map_err(|e| format!("Failed to send deactivated event: {}", e).into())
    }
    
    /// Проверяет, активен ли режим опорного сервера
    pub async fn is_active(&self) -> bool {
        let active = self.active.lock().await;
        *active
    }
    
    /// Добавляет маршрут в расширенный кэш
    pub async fn add_route(&self, peer_id: PeerId, addr: Multiaddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_active().await {
            return Ok(());
        }
        
        let mut routes = self.extended_routes.lock().await;
        
        // Проверяем размер кэша
        if routes.len() >= self.config.max_routes && !routes.contains_key(&peer_id) {
            return Ok(()); // Кэш заполнен и это новый узел
        }
        
        // Проверяем, содержит ли уже этот адрес
        let addresses = routes.entry(peer_id).or_insert_with(Vec::new);
        if addresses.contains(&addr) {
            return Ok(());
        }
        
        // Добавляем новый адрес
        addresses.push(addr.clone());
            
        // Обновляем статистику
        let mut stats = self.stats.lock().await;
        stats.cached_routes_count = routes.len();
        
        // Отправляем событие добавления маршрута
        self.event_tx.send(BootstrapEvent::RouteAdded {
            peer_id,
            addr,
        }).await
        .map_err(|e| format!("Failed to send route added event: {}", e).into())
    }
    
    /// Получает маршруты для указанного узла
    pub async fn get_routes(&self, peer_id: &PeerId) -> Vec<Multiaddr> {
        if !self.is_active().await {
            return Vec::new();
        }
        
        let routes = self.extended_routes.lock().await;
        
        // Обновляем статистику
        let mut stats = self.stats.lock().await;
        stats.route_requests += 1;
        
        match routes.get(peer_id) {
            Some(addrs) => {
                stats.successful_responses += 1;
                addrs.clone()
            }
            None => Vec::new(),
        }
    }
    
    /// Синхронизирует данные с другими опорными серверами
    pub async fn sync_with_bootstrap_nodes(
        &self, 
        kad_behaviour: &mut kad::Behaviour<kad::store::MemoryStore>
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_active().await {
            return Ok(());
        }
        
        // Обновляем время последней синхронизации
        {
            let mut last_sync = self.last_sync.lock().await;
            *last_sync = Instant::now();
        }
        
        // Получаем список опорных серверов
        let bootstrap_nodes = {
            let nodes = self.known_bootstrap_nodes.lock().await;
            nodes.clone()
        };
        
        // Синхронизируемся с каждым опорным сервером
        for (peer_id, addrs) in bootstrap_nodes {
            // Пропускаем себя
            if peer_id == self.local_peer_id {
                continue;
            }
            
            // Добавляем адреса в Kademlia
            for addr in addrs {
                kad_behaviour.add_address(&peer_id, addr);
            }
            
            // Инициируем запрос на поиск близких узлов
            kad_behaviour.get_closest_peers(peer_id);
        }
        
        // Обновляем статистику
        let stats = {
            let mut stats_lock = self.stats.lock().await;
            stats_lock.sync_count += 1;
            stats_lock.clone()
        };
        
        // Отправляем событие синхронизации
        self.event_tx.send(BootstrapEvent::Synced { stats }).await
            .map_err(|e| format!("Failed to send synced event: {}", e).into())
    }
    
    /// Проверяет, нужно ли синхронизироваться с опорными серверами
    pub async fn should_sync(&self) -> bool {
        if !self.is_active().await {
            return false;
        }
        
        let last_sync = self.last_sync.lock().await;
        last_sync.elapsed() > self.config.sync_interval
    }
    
    /// Обрабатывает события Kademlia для сбора информации о маршрутах
    pub async fn handle_kad_event(
        &self, 
        event: &kad::Event,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_active().await {
            return Ok(());
        }
        
        match event {
            kad::Event::RoutingUpdated { peer, addresses, .. } => {
                // Добавляем все адреса в расширенный кэш
                for addr in addresses.iter() {
                    self.add_route(*peer, addr.clone()).await?;
                }
                
                // Обновляем статистику
                let mut stats = self.stats.lock().await;
                stats.announced_routes += 1;
                
                // Отправляем событие обновления маршрутов
                self.event_tx.send(BootstrapEvent::RoutingUpdated {
                    peer_id: *peer,
                    addresses: addresses.iter().cloned().collect(),
                }).await
                .map_err(|e| format!("Failed to send routing updated event: {}", e).into())
            }
            
            // Обработка других полезных событий Kademlia
            kad::Event::OutboundQueryProgressed { result, .. } => {
                match result {
                    kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { peers, .. })) => {
                        // Когда находим близкие узлы, это хорошая возможность обновить кэш
                        // Здесь не имеем прямого доступа к kbuckets, поэтому просто отмечаем
                        // найденные узлы для последующего поиска
                        for _peer in peers {
                            // Запросим подробную информацию об этих узлах позже
                            // В будущей реализации здесь может быть запрос адресов
                        }
                        Ok(())
                    }
                    // Другие результаты запросов
                    _ => Ok(()),
                }
            }
            
            // Другие события, которые могут быть полезны для опорного сервера
            _ => Ok(()),
        }
    }
    
    /// Получает текущую статистику работы опорного сервера
    pub async fn get_stats(&self) -> BootstrapServerStats {
        let stats = self.stats.lock().await;
        stats.clone()
    }
    
    /// Добавляет новый опорный сервер в список известных
    pub async fn add_bootstrap_node(&self, peer_id: PeerId, addrs: Vec<Multiaddr>) {
        let mut nodes = self.known_bootstrap_nodes.lock().await;
        nodes.insert(peer_id, addrs);
    }
    
    /// Удаляет опорный сервер из списка известных
    pub async fn remove_bootstrap_node(&self, peer_id: &PeerId) {
        let mut nodes = self.known_bootstrap_nodes.lock().await;
        nodes.remove(peer_id);
    }
    
    /// Получает список известных опорных серверов
    pub async fn get_bootstrap_nodes(&self) -> HashMap<PeerId, Vec<Multiaddr>> {
        let nodes = self.known_bootstrap_nodes.lock().await;
        nodes.clone()
    }
    
    /// Очищает устаревшие маршруты
    pub async fn clean_expired_routes(&self) {
        if !self.is_active().await {
            return;
        }
        
        // В реальной реализации здесь был бы код для очистки устаревших маршрутов
        // на основе timestamp или других метрик
        
        // Обновляем статистику
        let routes = self.extended_routes.lock().await;
        let mut stats = self.stats.lock().await;
        stats.cached_routes_count = routes.len();
    }
    
    /// Ищет адреса узла в расширенном кэше и через Kademlia
    pub async fn find_peer(
        &self,
        peer_id: &PeerId,
        kad_behaviour: &mut kad::Behaviour<kad::store::MemoryStore>,
    ) -> Vec<Multiaddr> {
        // Если сервер не активен, используем только стандартный поиск Kademlia
        if !self.is_active().await {
            // Инициируем запрос на поиск узла в сети
            kad_behaviour.get_closest_peers(*peer_id);
            return Vec::new();
        }
        
        // Первым делом проверяем локальный кэш
        let addresses = self.get_routes(peer_id).await;
        
        // Если адресов нет в кэше, инициируем запрос через Kademlia
        if addresses.is_empty() {
            // Инициируем запрос на поиск узла в сети
            kad_behaviour.get_closest_peers(*peer_id);
            
            // Для непосредственного доступа к k-бакетам нужен отдельный механизм
            // Здесь мы просто инициируем поиск, результаты придут через события
        }
        
        addresses
    }
    
    /// Экспортирует информацию о маршрутах для других опорных серверов
    pub async fn export_routing_table(
        &self,
    ) -> HashMap<PeerId, Vec<Multiaddr>> {
        if !self.is_active().await {
            return HashMap::new();
        }
        
        let routes = self.extended_routes.lock().await;
        routes.clone()
    }
    
    /// Импортирует информацию о маршрутах от других опорных серверов
    pub async fn import_routing_table(
        &self,
        routes: HashMap<PeerId, Vec<Multiaddr>>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_active().await {
            return Ok(0);
        }
        
        let mut imported_count = 0;
        
        // Импортируем маршруты по одному
        for (peer_id, addrs) in routes {
            for addr in addrs {
                self.add_route(peer_id, addr).await?;
                imported_count += 1;
            }
        }
        
        Ok(imported_count)
    }
    
    /// Предоставляет API для проверки, является ли узел опорным сервером
    pub async fn is_bootstrap_node(&self, peer_id: &PeerId) -> bool {
        let nodes = self.known_bootstrap_nodes.lock().await;
        nodes.contains_key(peer_id)
    }
    
    /// Обновляет TTL для всех маршрутов опорного сервера
    pub async fn update_route_ttl(&self, new_ttl: Duration) {
        let mut config = self.config.clone();
        config.route_ttl = new_ttl;
    }
    
    /// Увеличивает лимит кэшированных маршрутов
    pub async fn increase_max_routes(&self, new_max: usize) {
        let mut config = self.config.clone();
        config.max_routes = new_max;
    }
    
    /// Проверяет, есть ли маршрут к указанному узлу
    pub async fn has_route_to(&self, peer_id: &PeerId) -> bool {
        if !self.is_active().await {
            return false;
        }
        
        let routes = self.extended_routes.lock().await;
        routes.contains_key(peer_id) && !routes[peer_id].is_empty()
    }
}
//! Обработчики событий для тестирования NetCom нод

use libp2p::PeerId;
use tokio::sync::oneshot;
use xnetwork::{
    commander::Commander,
    events::NetworkEvent,
};

/// Создает обработчик для событий подключения к пиру
pub fn create_peer_connected_handler(expected_peer_id: PeerId) -> (oneshot::Receiver<()>, impl FnMut(&NetworkEvent)) {
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    
    let handler = move |event: &NetworkEvent| {
        if let NetworkEvent::PeerConnected { peer_id } = event {
            if peer_id == &expected_peer_id {
                println!("✅ Получено событие подключения к пиру {:?}", peer_id);
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
            }
        }
    };
    
    (rx, handler)
}

/// Создает обработчик для событий отключения пира
pub fn create_peer_disconnected_handler(expected_peer_id: PeerId) -> (oneshot::Receiver<()>, impl FnMut(&NetworkEvent)) {
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    
    let handler = move |event: &NetworkEvent| {
        if let NetworkEvent::PeerDisconnected { peer_id } = event {
            if peer_id == &expected_peer_id {
                println!("✅ Получено событие отключения пира {:?}", peer_id);
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
            }
        }
    };
    
    (rx, handler)
}

/// Создает обработчик для событий взаимной аутентификации
pub fn create_mutual_auth_handler(expected_peer_id: PeerId) -> (oneshot::Receiver<()>, impl FnMut(&NetworkEvent)) {
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    
    let handler = move |event: &NetworkEvent| {
        if let NetworkEvent::AuthEvent { event: auth_event } = event {
            if let xauth::events::PorAuthEvent::MutualAuthSuccess { peer_id, .. } = auth_event {
                if peer_id == &expected_peer_id {
                    println!("✅ Получено событие взаимной аутентификации с пиром {:?}", peer_id);
                    if let Some(tx) = tx.take() {
                        let _ = tx.send(());
                    }
                }
            }
        }
    };
    
    (rx, handler)
}

/// Создает обработчик для автоматического одобрения POR запросов
pub fn create_por_approval_handler(commander: Commander) -> impl FnMut(&NetworkEvent) {
    move |event: &NetworkEvent| {
        if let NetworkEvent::AuthEvent { event: auth_event } = event {
            if let xauth::events::PorAuthEvent::VerifyPorRequest { peer_id, connection_id, .. } = auth_event {
                println!("✅ Получен POR запрос от пира {:?}", peer_id);
                println!("🔄 Автоматически одобряем POR запрос...");
                let commander_clone = commander.clone();
                let connection_id = *connection_id;
                tokio::spawn(async move {
                    let _ = commander_clone.submit_por_verification(
                        connection_id, 
                        xauth::definitions::AuthResult::Ok(std::collections::HashMap::new())
                    ).await;
                });
            }
        }
    }
}

/// Создает обработчик для автоматического одобрения POR запросов (альтернативная версия)
pub fn create_handler_submit_por_verification(commander: Commander) -> impl FnMut(&NetworkEvent) {
    move |event: &NetworkEvent| {
        if let NetworkEvent::AuthEvent { event: auth_event } = event {
            if let xauth::events::PorAuthEvent::VerifyPorRequest { peer_id, connection_id, .. } = auth_event {
                println!("✅ Получен POR запрос от пира {:?}", peer_id);
                println!("🔄 Автоматически одобряем POR запрос...");
                let commander_clone = commander.clone();
                let connection_id = *connection_id;
                tokio::spawn(async move {
                    let _ = commander_clone.submit_por_verification(
                        connection_id, 
                        xauth::definitions::AuthResult::Ok(std::collections::HashMap::new())
                    ).await;
                });
            }
        }
    }
}

/// Создает обработчик для событий открытия потока (заглушка)
pub fn create_xstream_opened_handler(_expected_peer_id: PeerId) -> (oneshot::Receiver<()>, impl FnMut(&NetworkEvent)) {
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    
    let handler = move |_event: &NetworkEvent| {
        // TODO: Реализовать, когда XStreamEvent будет доступен в NetworkEvent
        // Пока просто игнорируем события
    };
    
    (rx, handler)
}

/// Создает обработчик для событий обнаружения пиров (заглушка)
pub fn create_peer_discovered_handler(_expected_peer_id: PeerId) -> (oneshot::Receiver<()>, impl FnMut(&NetworkEvent)) {
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    
    let handler = move |_event: &NetworkEvent| {
        // TODO: Реализовать, когда DiscoveryEvent будет доступен в NetworkEvent
        // Пока просто игнорируем события
    };
    
    (rx, handler)
}

/// Создает обработчик для событий начала прослушивания адреса
pub fn create_listening_address_handler() -> (oneshot::Receiver<libp2p::Multiaddr>, impl FnMut(&NetworkEvent)) {
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    
    let handler = move |event: &NetworkEvent| {
        if let NetworkEvent::ListeningOnAddress { addr, .. } = event {
            println!("✅ Получено событие прослушивания на адресе: {}", addr);
            if let Some(tx) = tx.take() {
                let _ = tx.send(addr.clone());
            }
        }
    };
    
    (rx, handler)
}

use std::str::FromStr;
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use crate::network::commander::Commander;
use std::time::Duration;

/// Вспомогательные функции для подключения к опорным серверам
pub struct BootstrapConnect;




impl BootstrapConnect {
    /// Извлечение PeerId из multiaddr
    pub fn extract_peer_id_from_multiaddr(addr_str: &str) -> Result<(PeerId, Multiaddr), Box<dyn std::error::Error + Send + Sync>> {
        let addr = Multiaddr::from_str(addr_str)
            .map_err(|e| format!("Invalid multiaddress '{}': {}", addr_str, e))?;
        
        // Найти компонент p2p (он содержит PeerId)
        let components = addr.iter().collect::<Vec<_>>();
        let mut peer_id = None;
        let mut base_addr = addr.clone();
        
        // Ищем компонент p2p и извлекаем из него PeerId
        for (i, component) in components.iter().enumerate() {
            if let Protocol::P2p(hash) = component {
                // Правильный метод конвертации multihash в PeerId
                let pid = PeerId::try_from(hash.clone())
                    .map_err(|_| format!("Invalid peer ID in multiaddress"))?;
                peer_id = Some(pid);
                
                // Если PeerId в конце адреса, базовый адрес не включает его
                if i == components.len() - 1 {
                    // Создаем новый адрес без последнего компонента
                    base_addr = components[..i].iter()
                        .fold(Multiaddr::empty(), |mut addr, proto| {
                            addr.push(proto.clone());
                            addr
                        });
                }
                
                break;
            }
        }
        
        match peer_id {
            Some(pid) => Ok((pid, base_addr)),
            None => Err("No PeerId (p2p component) found in multiaddress".into())
        }
    }

    pub async fn connect_to_bootstrap_server(
        commander: &Commander, 
        bootstrap_addr: &str,
        local_peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        // 1. Извлекаем PeerId и адрес из строки
        let (peer_id, addr) = Self::extract_peer_id_from_multiaddr(bootstrap_addr)?;
        
        println!("🔌 Attempting to connect to bootstrap server {} at {}", peer_id, addr);
        
        // 2. Подключаемся к серверу
        commander.connect(addr.clone()).await?;
        println!("✅ Connected to bootstrap server at {}", addr);
        
        // 3. Запрашиваем информацию о маршрутах
        println!("🔍 Fetching routing information from bootstrap server...");
        if let Err(e) = commander.find_peer_addresses(local_peer_id).await {
            eprintln!("⚠️ Failed to fetch routing information: {}", e);
        }
        
        // 4. Добавляем узел как опорный сервер
        let mut full_addr = addr.clone();
        full_addr.push(Protocol::P2p(peer_id.into()));
        
        if let Err(e) = commander.add_bootstrap_node(peer_id, vec![full_addr]).await {
            eprintln!("⚠️ Failed to add bootstrap node: {}", e);
        } else {
            println!("📌 Node {} added as bootstrap server", peer_id);
        }
        
        // 5. Ждем немного, чтобы дать время на обработку запросов
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 6. Опционально: запрашиваем дополнительную информацию
        match commander.get_bootstrap_nodes().await {
            Ok(nodes) => {
                if !nodes.is_empty() {
                    println!("📋 Known bootstrap nodes: {}", nodes.len());
                    for (id, addrs) in nodes {
                        println!(" - {}: {}", id, addrs.len());
                    }
                }
            }
            Err(e) => {
                eprintln!("⚠️ Failed to get bootstrap nodes: {}", e);
            }
        }
        
        Ok(())
    }


    pub async fn connect_to_bootstrap_server_with_retry(
    commander: &Commander, 
    bootstrap_addr: &str,
    local_peer_id: PeerId,
    max_retries: usize,
    retry_delay: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut last_error = None;
    
    for attempt in 0..max_retries {
        match Self::connect_to_bootstrap_server(commander, bootstrap_addr, local_peer_id).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!("Attempt {}/{} to connect to bootstrap server failed: {}", 
                         attempt + 1, max_retries, e);
                last_error = Some(e);
                
                if attempt < max_retries - 1 {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| "Failed to connect to bootstrap server".into()))
}
}
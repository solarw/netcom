// Файл: ./src/network/commander.rs
use libp2p::{swarm::ConnectionId, Multiaddr, PeerId};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use super::bootstrap::{commands::BootstrapCommand, config::BootstrapServerStats};
use super::commands::NetworkCommand;
use super::xauth::definitions::AuthResult;
use super::xstream::manager::XStream;

pub struct Commander {
    cmd_tx: mpsc::Sender<NetworkCommand>,
}

impl Commander {
    pub fn new(cmd_tx: mpsc::Sender<NetworkCommand>) -> Commander {
        Commander { cmd_tx }
    }

    pub async fn listen_port(
        &self,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::OpenListenPort {
                port,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send open port command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(addr) => {
                println!("Server is listening on {}", addr);
                return Ok(());
            }
            Err(e) => {
                println!("Failed to listen: {}", e);
                return Err(format!("Failed to listen on port {}: {}", port, e).into());
            }
        }
    }

    pub async fn connect(
        &self,
        addr: Multiaddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Connect {
                addr,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send connect command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to connect: {}", e).into()),
        }
    }

    pub async fn disconnect(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Disconnect {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send disconnect command: {}", e).into()
            })?;

        match response_rx.await? {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to disconnect: {}", e).into()),
        }
    }

    pub async fn get_kad_known_peers(
        &self,
    ) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetKadKnownPeers {
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get kad peers command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    // Get known addresses for a peer from the Kademlia DHT
    pub async fn get_peer_addresses(
        &self,
        peer_id: PeerId,
    ) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::GetPeerAddresses {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get peer addresses command: {}", e).into()
            })?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e).into())
    }

    // Initiate a network search for a peer's addresses
    pub async fn find_peer_addresses(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::FindPeerAddresses {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send find peer addresses command: {}", e).into()
            })?;

        match response_rx.await {
            Ok(result) => result,
            Err(e) => Err(format!("Failed to receive response: {}", e).into()),
        }
    }

    pub async fn is_peer_authenticated(
        &self,
        peer_id: PeerId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::IsPeerAuthenticated {
                peer_id,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send authentication check command: {}", e).into()
            })?;

        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e).into())
    }

    // New method to submit PoR verification result
    pub async fn submit_por_verification(
        &self,
        connection_id: ConnectionId,
        result: AuthResult,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.cmd_tx
            .send(NetworkCommand::SubmitPorVerification {
                connection_id,
                result,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send PoR verification result: {}", e).into()
            })?;

        Ok(())
    }

    pub async fn open_stream(&self, peer_id: PeerId) -> Result<XStream, String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::OpenStream {
                peer_id: peer_id,
                connection_id: None,
                response: response_tx,
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send find peer addresses command: {}", e).into()
            });

        match response_rx.await {
            Ok(result) => result,
            Err(e) => Err(format!("Failed to receive response: {}", e).into()),
        }
    }

    // Методы для работы с опорным сервером

    // Активация опорного сервера
    pub async fn activate_bootstrap_server(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::Activate {
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send activate bootstrap server command: {}", e).into()
            })?;

        response_rx.await?
    }

    // Деактивация опорного сервера
    pub async fn deactivate_bootstrap_server(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::Deactivate {
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send deactivate bootstrap server command: {}", e).into()
            })?;

        response_rx.await?
    }

    // Получение статистики опорного сервера
    pub async fn get_bootstrap_server_stats(
        &self,
    ) -> Result<BootstrapServerStats, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::GetStats {
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get bootstrap server stats command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    // Добавление опорного сервера
    pub async fn add_bootstrap_node(
        &self,
        peer_id: PeerId,
        addrs: Vec<Multiaddr>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::AddNode {
                    peer_id,
                    addrs,
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send add bootstrap node command: {}", e).into()
            })?;

        response_rx.await?
    }

    // Удаление опорного сервера
    pub async fn remove_bootstrap_node(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::RemoveNode {
                    peer_id,
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send remove bootstrap node command: {}", e).into()
            })?;

        response_rx.await?
    }

    // Получение списка опорных серверов
    pub async fn get_bootstrap_nodes(
        &self,
    ) -> Result<HashMap<PeerId, Vec<Multiaddr>>, Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::GetNodes {
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send get bootstrap nodes command: {}", e).into()
            })?;

        Ok(response_rx.await?)
    }

    // Принудительная синхронизация с опорными серверами
    pub async fn force_bootstrap_sync(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.cmd_tx
            .send(NetworkCommand::Bootstrap {
                command: BootstrapCommand::ForceSync {
                    response: response_tx,
                },
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to send force bootstrap sync command: {}", e).into()
            })?;

        response_rx.await?
    }

    // Подключиться к опорному серверу по multiaddr
    pub async fn connect_to_bootstrap_node(
        &self,
        addr_str: &str,
    ) -> Result<PeerId, Box<dyn std::error::Error + Send + Sync>> {
        // Парсим multiaddr
        let addr = Multiaddr::from_str(addr_str)
            .map_err(|e| format!("Invalid multiaddress '{}': {}", addr_str, e))?;
        
        // Ищем компонент p2p и извлекаем из него PeerId
        let mut peer_id = None;
        
        for component in addr.iter() {
            if let libp2p::multiaddr::Protocol::P2p(hash) = component {
                // ИСПРАВЛЕНИЕ: правильный метод конвертации multihash в PeerId
                if let Ok(pid) = PeerId::try_from(hash.clone()) {
                    peer_id = Some(pid);
                } else {
                    return Err(format!("Invalid peer ID in multiaddress").into());
                }
                break;
            }
        }
        
        let peer_id = peer_id.ok_or("No PeerId (p2p component) found in multiaddress")?;
        
        // Подключаемся к опорному серверу
        self.connect(addr).await?;
        
        // Инициируем запрос на поиск ближайших узлов
        self.find_peer_addresses(peer_id).await?;
        
        Ok(peer_id)
    }
    

    // Найти и подключиться к узлу
    pub async fn find_and_connect_to_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Запускаем поиск адресов пира через DHT
        self.find_peer_addresses(peer_id).await?;

        // Даем время на распространение запроса
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Получаем найденные адреса
        let addresses = self.get_peer_addresses(peer_id).await?;

        if addresses.is_empty() {
            return Err("No addresses found for peer".into());
        }

        // Пробуем подключиться по каждому адресу
        for addr in addresses {
            match self.connect(addr.clone()).await {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => {
                    println!("Failed to connect to {} at {}: {}", peer_id, addr, e);
                    // Продолжаем с другими адресами
                }
            }
        }

        Err("Failed to connect to peer via any of its addresses".into())
    }



    pub async fn connect_to_bootstrap_server(
        &self,
        bootstrap_addr: &str,
        local_peer_id: PeerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Делегируем работу в BootstrapConnect
        crate::network::bootstrap::BootstrapConnect::connect_to_bootstrap_server(
            self, bootstrap_addr, local_peer_id
        ).await
    }
}

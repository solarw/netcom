use anyhow::{Context, Result};
use futures::StreamExt;
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use rand::RngCore;
use std::{io, sync::Arc, time::Duration};
use tokio::{
    select, sync::{mpsc, oneshot, Mutex}, 
    time::{sleep, Instant, interval}
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use xstream::{
    behaviour::XStreamNetworkBehaviour,
    events::XStreamEvent,
    xstream::XStream,
};

// Структура для хранения статистики узла
#[derive(Debug, Default, Clone)]
struct NodeStats {
    echo_requests_sent: usize,
    echo_responses_received: usize,
    successful_verifications: usize,
    failed_verifications: usize,
    bytes_sent: usize,
    bytes_received: usize,
    echo_requests_received: usize,
    echo_responses_sent: usize,
}

// Структура представляющая узел сети
struct Node {
    id: String,
    peer_id: PeerId,
    swarm: libp2p::Swarm<XStreamNetworkBehaviour>,
    listen_addr: Option<Multiaddr>,
    stats: Arc<Mutex<NodeStats>>,
    command_sender: mpsc::Sender<NodeCommand>,
    command_receiver: mpsc::Receiver<NodeCommand>,
    connected_peers: Vec<PeerId>,
}

// Команды для управления узлом
enum NodeCommand {
    Connect(Multiaddr),
    SendEcho(PeerId),
    GetInfo(oneshot::Sender<NodeInfo>),
    GetStats(oneshot::Sender<NodeStats>),
    Shutdown,
}

// Информация об узле
#[derive(Clone)]
struct NodeInfo {
    id: String,
    peer_id: PeerId,
    listen_addr: Option<Multiaddr>,
    connected_peers: Vec<PeerId>,
}

// Структура для результата проверки echo ответа
struct VerificationResult {
    data_verified: bool,
    bytes_sent: usize,
    bytes_received: usize,
    bytes_count: usize,
}

impl Node {
    // Создать новый узел с указанным портом для QUIC
    async fn new(id: &str, quic_port: u16, use_ipv6: bool) -> Result<Self> {
        // Создаем новый Swarm с XStreamNetworkBehaviour
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_quic()
            .with_behaviour(|_| XStreamNetworkBehaviour::new())
            .expect("Failed to create behavior")
            .build();

        // Начинаем прослушивать QUIC
        let listen_string = if use_ipv6 {
            format!("/ip6/::1/udp/{}/quic-v1", quic_port)
        } else {
            format!("/ip4/127.0.0.1/udp/{}/quic-v1", quic_port)
        };
        
        swarm.listen_on(listen_string.parse()?)?;

        let peer_id = *swarm.local_peer_id();
        let stats = Arc::new(Mutex::new(NodeStats::default()));
        let (command_sender, command_receiver) = mpsc::channel(10);

        Ok(Node {
            id: id.to_string(),
            peer_id,
            swarm,
            listen_addr: None,
            stats,
            command_sender,
            command_receiver,
            connected_peers: Vec::new(),
        })
    }

    // Получить канал для отправки команд узлу
    fn get_command_sender(&self) -> mpsc::Sender<NodeCommand> {
        self.command_sender.clone()
    }

    // Запустить узел
    async fn run(&mut self) {
        tracing::info!("Node {} (PeerId: {}) started", self.id, self.peer_id);

        // Интервал для периодического логирования статистики
        let mut log_stats_interval = interval(Duration::from_secs(10));

        loop {
            select! {
                // Обрабатываем события от swarm
                event = self.swarm.select_next_some() => {
                    match event {
                        libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                            let full_addr = address.clone().with_p2p(self.peer_id).unwrap();
                            tracing::info!("Node {} listening on {}", self.id, full_addr);
                            self.listen_addr = Some(address);
                        }
                        libp2p::swarm::SwarmEvent::Behaviour(XStreamEvent::IncomingStream { stream }) => {
                            // Сохраняем peer_id и запускаем echo обработчик
                            let peer_id = stream.peer_id;
                            let stats_clone = self.stats.clone();
                            let node_id = self.id.clone();
                            
                            tokio::spawn(async move {
                                match handle_echo_request(stream, stats_clone).await {
                                    Ok(bytes) => {
                                        tracing::info!("Node {} successfully echoed {} bytes to {}", node_id, bytes, peer_id);
                                    }
                                    Err(e) => {
                                        tracing::warn!("Node {} echo handler error: {}", node_id, e);
                                    }
                                }
                            });
                        }
                        libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            tracing::info!("Node {} established connection to {}", self.id, peer_id);
                            
                            // Обновляем статистику успешных подключений
                            // Добавляем пир в список подключенных, если его там еще нет
                            if !self.connected_peers.contains(&peer_id) {
                                self.connected_peers.push(peer_id);
                            }
                        }
                        libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            tracing::info!("Node {} closed connection to {}", self.id, peer_id);
                            
                            // Удаляем пир из списка подключенных
                            self.connected_peers.retain(|p| p != &peer_id);
                        }
                        libp2p::swarm::SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                            if let Some(peer) = peer_id {
                                tracing::warn!("Node {} failed to connect to {}: {}", self.id, peer, error);
                            }
                        }
                        _ => {}
                    }
                }
                // Обрабатываем команды
                cmd = self.command_receiver.recv() => {
                    if let Some(command) = cmd {
                        match command {
                            NodeCommand::Connect(addr) => {
                                tracing::info!("Node {} connecting to {}", self.id, addr);
                                
                                if let Err(e) = self.swarm.dial(addr.clone()) {
                                    tracing::error!("Node {} failed to dial {}: {}", self.id, addr, e);
                                }
                            }
                            NodeCommand::SendEcho(peer_id) => {
                                if !self.connected_peers.contains(&peer_id) {
                                    tracing::warn!("Node {} not connected to {}, can't send echo", self.id, peer_id);
                                } else {
                                    self.send_echo_to_peer(peer_id).await;
                                }
                            }
                            NodeCommand::GetInfo(response) => {
                                let info = NodeInfo {
                                    id: self.id.clone(),
                                    peer_id: self.peer_id,
                                    listen_addr: self.listen_addr.clone(),
                                    connected_peers: self.connected_peers.clone(),
                                };
                                let _ = response.send(info);
                            }
                            NodeCommand::GetStats(response) => {
                                let stats = self.stats.lock().await.clone();
                                let _ = response.send(stats);
                            }
                            NodeCommand::Shutdown => {
                                tracing::info!("Node {} shutting down", self.id);
                                break;
                            }
                        }
                    }
                }
                // Периодически логируем статистику
                _ = log_stats_interval.tick() => {
                    let stats = self.stats.lock().await;
                    tracing::info!(
                        "Node {} stats: sent={}, received={}, successful_verifications={}, failed_verifications={}, bytes_sent={}, bytes_received={}",
                        self.id,
                        stats.echo_requests_sent,
                        stats.echo_responses_received,
                        stats.successful_verifications,
                        stats.failed_verifications,
                        stats.bytes_sent,
                        stats.bytes_received
                    );
                }
            }
        }

        tracing::info!("Node {} stopped", self.id);
    }

    // Отправить echo запрос конкретному пиру
    async fn send_echo_to_peer(&mut self, peer_id: PeerId) {
        tracing::info!("Node {} sending echo to {}", self.id, peer_id);
        
        // Увеличиваем счетчик отправленных запросов
        {
            let mut stats = self.stats.lock().await;
            stats.echo_requests_sent += 1;
        }

        let (tx, rx) = oneshot::channel();
        self.swarm.behaviour_mut().open_stream(peer_id, tx).await;
        
        // Обработка результата в отдельной задаче
        let stats_clone = self.stats.clone();
        let node_id = self.id.clone();
        
        tokio::spawn(async move {
            match rx.await {
                Ok(Ok(stream)) => {
                    match send_echo(stream, stats_clone.clone()).await {
                        Ok(result) => {
                            let mut stats = stats_clone.lock().await;
                            stats.echo_responses_received += 1;
                            
                            if result.data_verified {
                                stats.successful_verifications += 1;
                                tracing::info!(
                                    "Node {} received correct echo response from {}: {} bytes",
                                    node_id, peer_id, result.bytes_count
                                );
                            } else {
                                stats.failed_verifications += 1;
                                tracing::warn!(
                                    "Node {} received INCORRECT echo response from {}: sent={}, received={}",
                                    node_id, peer_id, result.bytes_sent, result.bytes_received
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Node {} echo client error: {}", node_id, e);
                        }
                    }
                }
                Ok(Err(error)) => {
                    tracing::warn!("Node {} failed to open stream: {}", node_id, error);
                }
                Err(e) => {
                    tracing::warn!("Node {} channel error: {}", node_id, e);
                }
            }
        });
    }
}

// Обработчик входящего echo запроса
async fn handle_echo_request(stream: XStream, stats: Arc<Mutex<NodeStats>>) -> io::Result<usize> {
    let stream_id = stream.id;
    let mut total = 0;

    tracing::debug!("Starting echo handler for stream {:?}", stream_id);

    // Увеличиваем счетчик полученных запросов
    {
        let mut stats = stats.lock().await;
        stats.echo_requests_received += 1;
    }

    loop {
        // Читаем данные из потока
        match stream.read().await {
            Ok(data) => {
                if data.is_empty() {
                    tracing::debug!("Empty data from stream {:?}, closing write half", stream_id);
                    
                    // Закрываем пишущую часть потока явно
                    if let Err(e) = stream.write_eof().await {
                        tracing::error!("Error closing write half of stream {:?}: {:?}", stream_id, e);
                    }
                    
                    // Обновляем статистику отправленных ответов
                    let mut stats = stats.lock().await;
                    stats.echo_responses_sent += 1;
                    
                    return Ok(total);
                }
                
                let data_len = data.len();
                total += data_len;
                
                // Обновляем статистику по полученным байтам
                {
                    let mut stats = stats.lock().await;
                    stats.bytes_received += data_len;
                }
                
                // Отправляем данные обратно
                match stream.write_all(data).await {
                    Ok(_) => {
                        // Обновляем статистику по отправленным байтам
                        let mut stats = stats.lock().await;
                        stats.bytes_sent += data_len;
                    }
                    Err(e) => {
                        tracing::error!("Error writing to stream {:?}: {:?}", stream_id, e);
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    tracing::debug!("EOF received for stream {:?}", stream_id);
                    
                    // Закрываем пишущую часть потока явно при получении EOF
                    if let Err(e) = stream.write_eof().await {
                        tracing::error!("Error closing write half of stream {:?}: {:?}", stream_id, e);
                    }
                    
                    // Обновляем статистику отправленных ответов
                    let mut stats = stats.lock().await;
                    stats.echo_responses_sent += 1;
                    
                    return Ok(total);
                }
                
                return Err(e);
            }
        }
    }
}

// Отправить echo запрос и проверить ответ
async fn send_echo(mut stream: XStream, stats: Arc<Mutex<NodeStats>>) -> io::Result<VerificationResult> {
    let stream_id = stream.id;
    tracing::debug!("Starting echo client for stream {:?}", stream_id);
    
    // Используем простой способ для генерации случайных байт
    let num_bytes = 100 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() % 900) as usize;

    // Заполняем буфер случайными данными
    let mut bytes = vec![0; num_bytes];
    rand::thread_rng().fill_bytes(&mut bytes);
    
    // Отправляем данные
    stream.write_all(bytes.clone()).await?;
    
    // Обновляем статистику отправленных байтов
    {
        let mut stats = stats.lock().await;
        stats.bytes_sent += bytes.len();
    }
    
    // Закрываем пишущую часть потока
    stream.write_eof().await?;
    
    // Читаем ответ
    let mut response = Vec::new();
    
    // Собираем все данные из потока
    loop {
        match stream.read().await {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                
                response.extend_from_slice(&chunk);
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                
                return Err(e);
            }
        }
    }
    
    // Обновляем статистику полученных байтов
    {
        let mut stats = stats.lock().await;
        stats.bytes_received += response.len();
    }
    
    // Проверяем, что полученные данные совпадают с отправленными
    let data_verified = bytes == response;
    let result = VerificationResult {
        data_verified,
        bytes_sent: bytes.len(),
        bytes_received: response.len(),
        bytes_count: if data_verified { bytes.len() } else { 0 },
    };
    
    // Закрываем поток
    stream.close().await?;

    Ok(result)
}

// Структура для управления тестом
struct XStreamTester {
    node1: Option<Node>,
    node2: Option<Node>,
    node1_sender: mpsc::Sender<NodeCommand>,
    node2_sender: mpsc::Sender<NodeCommand>,
    test_duration: Duration,
    echo_interval: Duration,
}

impl XStreamTester {
    // Создать новый тестер с указанным количеством узлов
    async fn new(
        test_duration: Duration, 
        echo_interval: Duration,
        use_ipv6: bool,
        port1: u16,
        port2: u16
    ) -> Result<Self> {
        // Создаем первый узел
        let mut node1 = Node::new("node1", port1, use_ipv6).await?;
        let node1_sender = node1.get_command_sender();
        
        // Создаем второй узел
        let mut node2 = Node::new("node2", port2, use_ipv6).await?;
        let node2_sender = node2.get_command_sender();
        
        Ok(XStreamTester {
            node1: Some(node1),
            node2: Some(node2),
            node1_sender,
            node2_sender,
            test_duration,
            echo_interval,
        })
    }
    
    // Запустить оба узла и выполнить тестирование
    async fn run_test(&mut self) -> Result<()> {
        // Запускаем узлы в отдельных задачах
        let mut node1 = self.node1.take().unwrap();
        let node1_task = tokio::spawn(async move {
            node1.run().await;
        });
        
        let mut node2 = self.node2.take().unwrap();
        let node2_task = tokio::spawn(async move {
            node2.run().await;
        });
        
        // Ждем, пока узлы инициализируются и начнут прослушивание
        sleep(Duration::from_secs(1)).await;
        
        // Получаем информацию об узлах
        let node1_info = self.get_node_info(1).await?;
        let node2_info = self.get_node_info(2).await?;
        
        tracing::info!("Node1 listening on {:?} with peer_id {}", node1_info.listen_addr, node1_info.peer_id);
        tracing::info!("Node2 listening on {:?} with peer_id {}", node2_info.listen_addr, node2_info.peer_id);
        
        // Соединяем узлы
        if let Some(addr) = &node1_info.listen_addr {
            tracing::info!("Connecting node2 to node1...");
            self.node2_sender.send(NodeCommand::Connect(addr.clone())).await?;
        } else {
            anyhow::bail!("Node1 has no listen address");
        }
        
        // Ждем установления соединения
        sleep(Duration::from_secs(2)).await;
        
        // Запускаем цикл тестирования
        tracing::info!("Starting echo test for {} seconds", self.test_duration.as_secs());
        let start_time = Instant::now();
        let mut echo_timer = interval(self.echo_interval);
        
        while start_time.elapsed() < self.test_duration {
            echo_timer.tick().await;
            
            // Отправляем эхо-запросы между узлами
            self.node1_sender.send(NodeCommand::SendEcho(node2_info.peer_id)).await?;
            self.node2_sender.send(NodeCommand::SendEcho(node1_info.peer_id)).await?;
        }
        
        // Собираем статистику
        let node1_stats = self.get_node_stats(1).await?;
        let node2_stats = self.get_node_stats(2).await?;
        
        tracing::info!(
            "Node1 stats: sent={}, received={}, successful_verifications={}, failed_verifications={}, bytes_sent={}, bytes_received={}",
            node1_stats.echo_requests_sent,
            node1_stats.echo_responses_received,
            node1_stats.successful_verifications,
            node1_stats.failed_verifications,
            node1_stats.bytes_sent,
            node1_stats.bytes_received
        );
        
        tracing::info!(
            "Node2 stats: sent={}, received={}, successful_verifications={}, failed_verifications={}, bytes_sent={}, bytes_received={}",
            node2_stats.echo_requests_sent,
            node2_stats.echo_responses_received,
            node2_stats.successful_verifications,
            node2_stats.failed_verifications,
            node2_stats.bytes_sent,
            node2_stats.bytes_received
        );
        
        // Завершаем узлы
        self.node1_sender.send(NodeCommand::Shutdown).await?;
        self.node2_sender.send(NodeCommand::Shutdown).await?;
        
        // Ждем завершения задач
        let _ = tokio::join!(node1_task, node2_task);
        
        Ok(())
    }
    
    // Получить информацию об узле
    async fn get_node_info(&self, node_num: usize) -> Result<NodeInfo> {
        let (tx, rx) = oneshot::channel();
        
        if node_num == 1 {
            self.node1_sender.send(NodeCommand::GetInfo(tx)).await?;
        } else {
            self.node2_sender.send(NodeCommand::GetInfo(tx)).await?;
        }
        
        rx.await.context("Failed to get node info")
    }
    
    // Получить статистику узла
    async fn get_node_stats(&self, node_num: usize) -> Result<NodeStats> {
        let (tx, rx) = oneshot::channel();
        
        if node_num == 1 {
            self.node1_sender.send(NodeCommand::GetStats(tx)).await?;
        } else {
            self.node2_sender.send(NodeCommand::GetStats(tx)).await?;
        }
        
        rx.await.context("Failed to get node stats")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Инициализируем логирование
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()
                .unwrap_or_else(|_| EnvFilter::default()),
        )
        .init();

    // Настройки тестирования
    let test_duration = Duration::from_secs(30);
    let echo_interval = Duration::from_secs(1);
    let use_ipv6 = false;
    let port1 = 10000;
    let port2 = 10001;
    
    // Создаем тестер
    tracing::info!("Creating XStream tester...");
    let mut tester = XStreamTester::new(
        test_duration,
        echo_interval,
        use_ipv6,
        port1,
        port2
    ).await?;
    
    // Запускаем тест
    tracing::info!("Starting XStream test...");
    tester.run_test().await?;
    
    tracing::info!("Test completed successfully!");
    Ok(())
}
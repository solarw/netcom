use anyhow::{Context, Result};
use futures::StreamExt;
use libp2p::{multiaddr::Protocol, Multiaddr};
use rand::RngCore;
use std::{io, sync::Arc, time::Duration};
use tokio::{sync::{oneshot, Mutex}, time::Instant};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use xstream::{
    behaviour::XStreamNetworkBehaviour,
    events::XStreamEvent,
    xstream::XStream,
};

// Структура для отслеживания состояния соединения
struct ConnectionState {
    is_connected: bool,
    last_connection_time: Instant,
    peer_address: Option<Multiaddr>,
    peer_id: Option<libp2p::PeerId>,
    last_echo_time: Instant,
    need_echo: bool,
    echo_success_count: usize,  // Счетчик успешных echo
    echo_failure_count: usize,  // Счетчик неудачных echo
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .init();

    let maybe_address = std::env::args()
        .nth(1)
        .map(|arg| arg.parse::<Multiaddr>())
        .transpose()
        .context("Failed to parse argument as `Multiaddr`")?;

    // Создаем новый Swarm с XStreamNetworkBehaviour
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| XStreamNetworkBehaviour::new())
        .expect("Failed to create behavior")
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse()?)?;

    // Инициализируем состояние соединения
    let connection_state = Arc::new(Mutex::new(ConnectionState {
        is_connected: false,
        last_connection_time: Instant::now() - Duration::from_secs(10),
        peer_address: maybe_address.clone(),
        peer_id: if let Some(address) = &maybe_address {
            if let Some(Protocol::P2p(peer_id)) = address.iter().last() {
                Some(peer_id)
            } else {
                None
            }
        } else {
            None
        },
        last_echo_time: Instant::now() - Duration::from_secs(10),
        need_echo: false,
        echo_success_count: 0,
        echo_failure_count: 0,
    }));

    // Опрашиваем swarm для продвижения вперед
    loop {
        // Проверяем и обновляем состояние соединения
        let should_dial = {
            let mut state = connection_state.lock().await;
            let now = Instant::now();
            
            // Если подключены и прошло более 5 секунд от последнего подключения
            if state.is_connected && now.duration_since(state.last_connection_time) > Duration::from_secs(5) {
                tracing::info!(
                    "Closing connection after 5 seconds... Echo stats: {} successful, {} failed",
                    state.echo_success_count, 
                    state.echo_failure_count
                );
                state.is_connected = false;
                state.need_echo = false;
                true // нужно переподключиться
            } else if !state.is_connected && state.peer_id.is_some() && state.peer_address.is_some() {
                true // нужно подключиться
            } else {
                false // ничего не делаем
            }
        };

        // Подключаемся, если необходимо
        if should_dial {
            let state = connection_state.lock().await;
            if let (Some(peer_id), Some(address)) = (state.peer_id, state.peer_address.clone()) {
                tracing::info!("Dialing peer: {}", peer_id);
                if let Err(e) = swarm.dial(address.clone()) {
                    tracing::warn!("Failed to dial: {}", e);
                }
            }
        }

        // Проверяем, нужно ли отправить эхо
        let should_send_echo = {
            let mut state = connection_state.lock().await;
            let now = Instant::now();
            
            if state.is_connected && 
               state.need_echo && 
               now.duration_since(state.last_echo_time) >= Duration::from_secs(1) {
                state.last_echo_time = now;
                state.need_echo = false;
                true
            } else {
                false
            }
        };

        // Отправляем эхо, если необходимо
        if should_send_echo {
            let state = connection_state.lock().await;
            if let Some(peer_id) = state.peer_id {
                let (tx, rx) = oneshot::channel();
                swarm.behaviour_mut().open_stream(peer_id, tx).await;
                
                // Обработка результата в отдельной задаче
                let peer_id_clone = peer_id;
                let connection_state_clone = connection_state.clone();
                
                tokio::spawn(async move {
                    match rx.await {
                        Ok(Ok(stream)) => {
                            match send(stream).await {
                                Ok(verification_result) => {
                                    let mut state = connection_state_clone.lock().await;
                                    if verification_result.data_verified {
                                        tracing::info!(
                                            peer = %peer_id_clone,
                                            "Echo complete! Data verification: SUCCESS ({} bytes matched)",
                                            verification_result.bytes_count
                                        );
                                        state.echo_success_count += 1;
                                    } else {
                                        tracing::warn!(
                                            peer = %peer_id_clone,
                                            "Echo complete but data verification FAILED! Expected {} bytes, got {} bytes",
                                            verification_result.bytes_sent,
                                            verification_result.bytes_received
                                        );
                                        state.echo_failure_count += 1;
                                    }
                                    
                                    // Разрешаем следующую отправку эхо
                                    state.need_echo = true;
                                }
                                Err(e) => {
                                    tracing::warn!(peer = %peer_id_clone, "Echo protocol failed: {e}");
                                    let mut state = connection_state_clone.lock().await;
                                    state.echo_failure_count += 1;
                                    state.need_echo = true; // Все равно разрешаем следующую попытку
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            tracing::info!(peer = %peer_id_clone, %error, "Failed to open stream");
                            let mut state = connection_state_clone.lock().await;
                            state.echo_failure_count += 1;
                            state.need_echo = true; // Разрешаем попробовать снова
                        }
                        Err(e) => {
                            tracing::warn!(peer = %peer_id_clone, "Channel error: {}", e);
                            let mut state = connection_state_clone.lock().await;
                            state.echo_failure_count += 1;
                            state.need_echo = true; // Разрешаем попробовать снова
                        }
                    }
                });
            }
        }

        // Используем timeout, чтобы опрашивать swarm с интервалами
        match tokio::time::timeout(Duration::from_millis(100), swarm.next()).await {
            Ok(Some(event)) => {
                match event {
                    libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                        let listen_address = address.with_p2p(*swarm.local_peer_id()).unwrap();
                        tracing::info!(%listen_address);
                    }
                    libp2p::swarm::SwarmEvent::Behaviour(XStreamEvent::IncomingStream { stream }) => {
                        // Сохраняем peer_id перед передачей stream в функцию echo
                        let peer_id = stream.peer_id;
                        
                        // Обрабатываем входящий поток в отдельной задаче
                        tokio::spawn(async move {
                            match echo(stream).await {
                                Ok(n) => {
                                    tracing::info!(peer = %peer_id, "Echoed {n} bytes!");
                                }
                                Err(e) => {
                                    tracing::warn!(peer = %peer_id, "Echo failed: {e}");
                                }
                            }
                        });
                    }
                    libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        tracing::info!("Connection established with {peer_id}");
                        
                        // Обновляем состояние соединения
                        let mut state = connection_state.lock().await;
                        state.is_connected = true;
                        state.last_connection_time = Instant::now();
                        state.need_echo = true; // Разрешаем отправку эхо
                    }
                    libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        tracing::info!("Connection closed with {peer_id}");
                        
                        // Обновляем состояние соединения
                        let mut state = connection_state.lock().await;
                        state.is_connected = false;
                        state.need_echo = false;
                    }
                    event => tracing::trace!(?event),
                }
            }
            Ok(None) => {
                // Swarm закрылся - завершаем программу
                break;
            }
            Err(_) => {
                // Таймаут - продолжаем цикл
                continue;
            }
        }
    }

    Ok(())
}

/// Функция эхо: читает данные из потока и отправляет их обратно
async fn echo(mut stream: XStream) -> io::Result<usize> {
    let stream_id = stream.id;
    let mut total = 0;

    tracing::info!("Starting echo for stream {:?}", stream_id);

    loop {
        // Читаем данные из потока
        match stream.read().await {
            Ok(data) => {
                tracing::info!("Read {} bytes from stream {:?}", data.len(), stream_id);
                
                if data.is_empty() {
                    tracing::info!("Empty data from stream {:?}, closing write half", stream_id);
                    // Закрываем пишущую часть потока явно
                    if let Err(e) = stream.write_eof().await {
                        tracing::error!("Error closing write half of stream {:?}: {:?}", stream_id, e);
                    }
                    return Ok(total);
                }
                
                total += data.len();
                
                // Отправляем данные обратно
                tracing::info!("Writing {} bytes back to stream {:?}", data.len(), stream_id);
                match stream.write_all(data).await {
                    Ok(_) => tracing::info!("Successfully wrote data back to stream {:?}", stream_id),
                    Err(e) => {
                        tracing::error!("Error writing to stream {:?}: {:?}", stream_id, e);
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                tracing::info!("Error reading from stream {:?}: {:?}", stream_id, e);
                
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    tracing::info!("EOF received for stream {:?}, closing write half", stream_id);
                    // Закрываем пишущую часть потока явно при получении EOF
                    if let Err(e) = stream.write_eof().await {
                        tracing::error!("Error closing write half of stream {:?}: {:?}", stream_id, e);
                    }
                    return Ok(total);
                }
                
                return Err(e);
            }
        }
    }
}

// Структура для возвращения результатов проверки данных
struct VerificationResult {
    data_verified: bool,
    bytes_sent: usize,
    bytes_received: usize,
    bytes_count: usize,
}

/// Отправляет случайные данные и проверяет, что они корректно возвращаются
async fn send(mut stream: XStream) -> io::Result<VerificationResult> {
    let stream_id = stream.id;
    tracing::info!("Starting send for stream {:?}", stream_id);
    
    // Используем простой способ для генерации случайных байт
    let num_bytes = 100 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() % 900) as usize;

    // Заполняем буфер случайными данными
    let mut bytes = vec![0; num_bytes];
    // Используем новый метод rand::rng() вместо thread_rng()
    rand::rng().fill_bytes(&mut bytes);
    
    tracing::info!("Sending {} bytes to stream {:?}", bytes.len(), stream_id);
    
    // Отправляем данные
    stream.write_all(bytes.clone()).await?;
    tracing::info!("Successfully sent data to stream {:?}", stream_id);
    
    // Закрываем пишущую часть потока
    tracing::info!("Closing write half of stream {:?}", stream_id);
    stream.write_eof().await?;
    tracing::info!("Successfully closed write half of stream {:?}", stream_id);
    
    // Читаем ответ
    let mut response = Vec::new();
    
    // Собираем все данные из потока
    tracing::info!("Reading response from stream {:?}", stream_id);
    loop {
        match stream.read().await {
            Ok(chunk) => {
                tracing::info!("Read chunk of {} bytes from stream {:?}", chunk.len(), stream_id);
                
                if chunk.is_empty() {
                    tracing::info!("Empty chunk from stream {:?}, breaking", stream_id);
                    break;
                }
                
                response.extend_from_slice(&chunk);
            }
            Err(e) => {
                tracing::info!("Error reading from stream {:?}: {:?}", stream_id, e);
                
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    tracing::info!("EOF received from stream {:?}, breaking", stream_id);
                    break;
                }
                
                return Err(e);
            }
        }
    }
    
    tracing::info!("Received total {} bytes from stream {:?}", response.len(), stream_id);
    
    // Проверяем, что полученные данные совпадают с отправленными
    let data_verified = bytes == response;
    let result = VerificationResult {
        data_verified,
        bytes_sent: bytes.len(),
        bytes_received: response.len(),
        bytes_count: if data_verified { bytes.len() } else { 0 },
    };
    
    if result.data_verified {
        tracing::info!("Echo response VERIFIED for stream {:?} ({} bytes matched)", stream_id, result.bytes_count);
    } else {
        // Детальное логирование ошибки
        tracing::error!(
            "Echo response MISMATCH for stream {:?}: sent {} bytes, received {} bytes",
            stream_id, 
            result.bytes_sent,
            result.bytes_received
        );
        
        // Дополнительные проверки для выявления характера ошибки
        if result.bytes_sent != result.bytes_received {
            tracing::error!("Length mismatch: expected {} bytes, got {} bytes", result.bytes_sent, result.bytes_received);
        } else {
            // Находим индекс первого различия
            for (i, (sent, received)) in bytes.iter().zip(response.iter()).enumerate() {
                if sent != received {
                    tracing::error!("First difference at byte {}: sent {}, received {}", i, sent, received);
                    break;
                }
            }
        }
    }
    
    // Закрываем поток
    tracing::info!("Closing stream {:?}", stream_id);
    stream.close().await?;
    tracing::info!("Successfully closed stream {:?}", stream_id);

    Ok(result)
}
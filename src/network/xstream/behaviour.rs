use super::consts::XSTREAM_PROTOCOL;
use std::collections::HashMap;
use libp2p::{
    StreamProtocol,
    Multiaddr,
    PeerId,
    swarm::{
        ConnectionId, 
        NetworkBehaviour, 
        NotifyHandler, 
        ToSwarm,
        derive_prelude::*,
    },
    core::transport::PortUse,
};
use tokio::sync::oneshot;

use super::events::XStreamEvent;
use super::handler::{XStreamHandler, XStreamHandlerEvent, XStreamHandlerIn};
use super::xstream::XStream;

/// NetworkBehaviour для работы с XStream
pub struct XStreamNetworkBehaviour {
    /// Отображение (peer_id, stream_id) -> XStream
    streams: HashMap<(PeerId, u128), XStream>,
    /// События, ожидающие обработки
    events: Vec<ToSwarm<XStreamEvent, XStreamHandlerIn>>,
    /// Ожидающие открытия потоки
    pending_streams: HashMap<PeerId, oneshot::Sender<Result<XStream, String>>>,
}

impl XStreamNetworkBehaviour {
    /// Создает новый XStreamNetworkBehaviour
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            events: Vec::new(),
            pending_streams: HashMap::new(),
        }
    }

    /// Открывает новый поток к указанному пиру
    pub fn request_open_stream(&mut self, peer_id: PeerId) {
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: XStreamHandlerIn::OpenStream,
        });
    }

    /// Асинхронно открывает новый поток и возвращает XStream или ошибку
    pub async fn open_stream(&mut self, peer_id: PeerId) -> Result<XStream, String> {
        // Создаем канал для получения результата
        let (sender, receiver) = tokio::sync::oneshot::channel();
        
        // Добавляем ожидание потока в карту ожидающих потоков
        self.pending_streams.insert(peer_id, sender);
        
        // Запрашиваем открытие потока
        self.request_open_stream(peer_id);
        
        // Ожидаем результат с таймаутом
        match tokio::time::timeout(std::time::Duration::from_secs(10), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Канал был закрыт без отправки результата".to_string()),
            Err(_) => Err("Таймаут при открытии потока".to_string()),
        }
    }

    /// Получает XStream по идентификатору пира и потока
    pub fn get_stream(&self, peer_id: PeerId, stream_id: u128) -> Option<&XStream> {
        self.streams.get(&(peer_id, stream_id))
    }

    /// Получает изменяемый XStream по идентификатору пира и потока
    pub fn get_stream_mut(&mut self, peer_id: PeerId, stream_id: u128) -> Option<&mut XStream> {
        self.streams.get_mut(&(peer_id, stream_id))
    }

    /// Закрывает поток с указанным идентификатором пира и потока
    pub async fn close_stream(&mut self, peer_id: PeerId, stream_id: u128) -> Result<(), std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            let result = stream.close().await;
            // Событие о закрытии потока будет отправлено из handler'а
            return result;
        }
        Ok(())
    }

    /// Отправляет данные в указанный поток
    pub async fn send_data(&mut self, peer_id: PeerId, stream_id: u128, data: Vec<u8>) -> Result<(), std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.write_all(data).await;
        }
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Stream not found"))
    }

    /// Читает данные из указанного потока
    pub async fn read_data(&mut self, peer_id: PeerId, stream_id: u128) -> Result<Vec<u8>, std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.read().await;
        }
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Stream not found"))
    }

    /// Читает точное количество байт из указанного потока
    pub async fn read_exact(&mut self, peer_id: PeerId, stream_id: u128, size: usize) -> Result<Vec<u8>, std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.read_exact(size).await;
        }
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Stream not found"))
    }

    /// Читает все данные из указанного потока до конца
    pub async fn read_to_end(&mut self, peer_id: PeerId, stream_id: u128) -> Result<Vec<u8>, std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.read_to_end().await;
        }
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Stream not found"))
    }
}

impl NetworkBehaviour for XStreamNetworkBehaviour {
    type ConnectionHandler = XStreamHandler;
    type ToSwarm = XStreamEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let mut handler = XStreamHandler::new();
        // Сразу устанавливаем peer_id в handler
        handler.set_peer_id(peer);
        Ok(handler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
        _port_use: PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let mut handler = XStreamHandler::new();
        // Сразу устанавливаем peer_id в handler
        handler.set_peer_id(peer);
        Ok(handler)
    }

    fn on_swarm_event(&mut self, _event: libp2p::swarm::FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        _connection_id: ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        match event {
            XStreamHandlerEvent::StreamEstablished { stream_id, stream } => {
                // Сохраняем поток в HashMap
                self.streams.insert((peer_id, stream_id), stream.clone());

                // Проверяем, есть ли ожидающий отправитель для этого пира
                if let Some(sender) = self.pending_streams.remove(&peer_id) {
                    // Отправляем успешный результат
                    let _ = sender.send(Ok(stream.clone()));
                }

                // Отправляем событие о новом потоке
                self.events.push(ToSwarm::GenerateEvent(XStreamEvent::StreamEstablished {
                    peer_id,
                    stream_id,
                }));
                
                // Также отправляем событие IncomingStream для обратной совместимости
                self.events.push(ToSwarm::GenerateEvent(XStreamEvent::IncomingStream {
                    stream,
                }));
            }
            XStreamHandlerEvent::StreamError { stream_id, error } => {
                // Если известен stream_id, удаляем поток из HashMap
                if let Some(stream_id) = stream_id {
                    self.streams.remove(&(peer_id, stream_id));
                }

                // Отправляем событие об ошибке
                self.events.push(ToSwarm::GenerateEvent(XStreamEvent::StreamError {
                    peer_id,
                    stream_id,
                    error,
                }));
            }
            XStreamHandlerEvent::StreamClosed { stream_id } => {
                // Удаляем поток из HashMap
                self.streams.remove(&(peer_id, stream_id));

                // Отправляем событие о закрытии потока
                self.events.push(ToSwarm::GenerateEvent(XStreamEvent::StreamClosed {
                    peer_id,
                    stream_id,
                }));
            }
        }
    }

    fn poll(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        if let Some(event) = self.events.pop() {
            return std::task::Poll::Ready(event);
        }

        std::task::Poll::Pending
    }
}
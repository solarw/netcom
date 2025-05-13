use std::task::{Context, Poll};

use super::xstream::XStream;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use libp2p::{
    swarm::{
        handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
        ConnectionHandler, ConnectionHandlerEvent, SubstreamProtocol,
    },
    PeerId, Stream, StreamProtocol,
};

use super::consts::XSTREAM_PROTOCOL;
use super::utils::IdIterator;

/// Возможные события, которые handler может отправить в behaviour
#[derive(Debug)]
pub enum XStreamHandlerEvent {
    /// Установлен новый stream
    IncomingStreamEstablished {
        /// Идентификатор потока
        stream_id: u128,
        /// Поток XStream
        stream: XStream,
    },
    OutboundStreamEstablished {
        /// Идентификатор потока
        stream_id: u128,
        /// Поток XStream
        stream: XStream,
    },
    /// Произошла ошибка при работе со stream
    StreamError {
        /// Идентификатор потока (если известен)
        stream_id: Option<u128>,
        /// Описание ошибки
        error: String,
    },
    /// Поток XStream закрыт
    StreamClosed {
        /// Идентификатор потока
        stream_id: u128,
    },
}

/// Команды, которые behaviour может отправить в handler
#[derive(Debug)]
pub enum XStreamHandlerIn {
    /// Открыть новый поток
    OpenStream,
}

/// Простая реализация протокола потока
#[derive(Debug, Clone)]
pub struct XStreamProtocol {
    protocol: StreamProtocol,
}

impl XStreamProtocol {
    /// Создает новый протокол XStream
    pub fn new(protocol: StreamProtocol) -> Self {
        Self { protocol }
    }
}

/// Handler для XStream
pub struct XStreamHandler {
    /// Итератор для генерации уникальных ID
    id_iter: IdIterator,
    /// Активные потоки
    streams: Vec<XStream>,
    /// Очередь исходящих событий
    outgoing_events: Vec<XStreamHandlerEvent>,
    /// Очередь входящих команд
    pending_commands: Vec<XStreamHandlerIn>,
    /// Peer ID для потоков
    peer_id: PeerId,
}

impl XStreamHandler {
    /// Создает новый XStreamHandler
    pub fn new() -> Self {
        // Используем дефолтный PeerId, который будет заменен позже
        Self {
            id_iter: IdIterator::new(),
            streams: Vec::new(),
            outgoing_events: Vec::new(),
            pending_commands: Vec::new(),
            peer_id: PeerId::random(),
        }
    }

    /// Устанавливает peer_id для handler
    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = peer_id;
    }

    /// Получает изменяемый XStream по его ID
    pub fn get_stream_mut(&mut self, stream_id: u128) -> Option<&mut XStream> {
        self.streams.iter_mut().find(|s| s.id == stream_id)
    }

    /// Обрабатывает новый входящий поток
    fn handle_inbound_stream(&mut self, stream: Stream, protocol: StreamProtocol) {
        println!("INBOUND");
        let stream_id = self.id_iter.next().unwrap();
        let (read, write) = AsyncReadExt::split(stream);

        let xstream = XStream::new(stream_id, self.peer_id, read, write);

        self.streams.push(xstream.clone());
        self.outgoing_events
            .push(XStreamHandlerEvent::IncomingStreamEstablished {
                stream_id,
                stream: xstream,
            });
    }

    fn handle_outbound_stream(&mut self, stream: Stream, protocol: StreamProtocol) {
        println!("OUTBOUND");
        let stream_id = self.id_iter.next().unwrap();
        let (read, write) = AsyncReadExt::split(stream);
    
        let xstream = XStream::new(stream_id, self.peer_id, read, write);
        self.streams.push(xstream.clone());
        self.outgoing_events
            .push(XStreamHandlerEvent::OutboundStreamEstablished {
                stream_id, // Use the correct stream_id instead of 0
                stream: xstream,
            });
    }

    /// Открывает новый исходящий поток
    fn open_outbound_stream(&mut self) -> SubstreamProtocol<XStreamProtocol, ()> {
        // Создаем протокол с нашим StreamProtocol
        let proto = XStreamProtocol::new(XSTREAM_PROTOCOL.clone());
        SubstreamProtocol::new(proto, ())
    }

    /// Получает XStream по его ID
    pub fn get_stream(&self, stream_id: u128) -> Option<&XStream> {
        self.streams.iter().find(|s| s.id == stream_id)
    }
}

// Реализация upgrade для нашего протокола
impl libp2p::core::upgrade::InboundUpgrade<Stream> for XStreamProtocol {
    type Output = (Stream, StreamProtocol);
    type Error = std::io::Error;
    type Future = futures::future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, stream: Stream, _info: StreamProtocol) -> Self::Future {
        futures::future::ready(Ok((stream, self.protocol.clone())))
    }
}

// Реализация upgrade для нашего протокола
impl libp2p::core::upgrade::OutboundUpgrade<Stream> for XStreamProtocol {
    type Output = (Stream, StreamProtocol);
    type Error = std::io::Error;
    type Future = futures::future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, stream: Stream, _info: StreamProtocol) -> Self::Future {
        futures::future::ready(Ok((stream, self.protocol.clone())))
    }
}

impl libp2p::core::upgrade::UpgradeInfo for XStreamProtocol {
    type Info = StreamProtocol;
    type InfoIter = std::iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once(self.protocol.clone())
    }
}

impl ConnectionHandler for XStreamHandler {
    type FromBehaviour = XStreamHandlerIn;
    type ToBehaviour = XStreamHandlerEvent;
    type InboundProtocol = XStreamProtocol;
    type OutboundProtocol = XStreamProtocol;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        // Создаем протокол с нашим StreamProtocol
        let proto = XStreamProtocol::new(XSTREAM_PROTOCOL.clone());
        SubstreamProtocol::new(proto, ())
    }

    fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
        match event {
            XStreamHandlerIn::OpenStream => {
                self.pending_commands.push(XStreamHandlerIn::OpenStream);
            }
        }
    }

    fn connection_keep_alive(&self) -> bool {
        !self.streams.is_empty() // Uncomment this line
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>,
    > {
        // Обрабатываем исходящие события
        if !self.outgoing_events.is_empty() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                self.outgoing_events.remove(0),
            ));
        }

        // Обрабатываем входящие команды
        if !self.pending_commands.is_empty() {
            match self.pending_commands.remove(0) {
                XStreamHandlerIn::OpenStream => {
                    return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                        protocol: self.open_outbound_stream(),
                    });
                }
            }
        }

        // Нет событий для обработки
        Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<Self::InboundProtocol, Self::OutboundProtocol>,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(FullyNegotiatedInbound {
                protocol: (stream, protocol),
                info: (),
            }) => {
                self.handle_inbound_stream(stream, protocol);
            }
            ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
                protocol: (stream, protocol),
                info: (),
            }) => {
                // Обрабатываем исходящий поток аналогично входящему
                self.handle_outbound_stream(stream, protocol);
            }
            ConnectionEvent::ListenUpgradeError(error) => {
                self.outgoing_events.push(XStreamHandlerEvent::StreamError {
                    stream_id: None,
                    error: format!("Listen upgrade error: {:?}", error.error),
                });
            }
            ConnectionEvent::DialUpgradeError(error) => {
                self.outgoing_events.push(XStreamHandlerEvent::StreamError {
                    stream_id: None,
                    error: format!("Dial upgrade error: {:?}", error.error),
                });
            }
            _ => {
                // Остальные события не представляют интереса для XStream
            }
        }
    }

    fn poll_close(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Self::ToBehaviour>> {
        // Если есть исходящие события, отправляем их
        if !self.outgoing_events.is_empty() {
            return Poll::Ready(Some(self.outgoing_events.remove(0)));
        }

        // Отмечаем все потоки как закрытые перед завершением
        if !self.streams.is_empty() {
            let stream_ids: Vec<u128> = self.streams.iter().map(|s| s.id).collect();
            self.streams.clear();

            for stream_id in stream_ids {
                return Poll::Ready(Some(XStreamHandlerEvent::StreamClosed { stream_id }));
            }
        }

        // Иначе сигнализируем о завершении
        Poll::Ready(None)
    }
}

use std::task::{Context, Poll};

use super::header::{read_header_from_stream, write_header_to_stream, XStreamHeader};
use super::types::{SubstreamRole, XStreamDirection, XStreamID, XStreamIDIterator};
use super::xstream::XStream;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::{
    swarm::{
        handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
        ConnectionHandler, ConnectionHandlerEvent, SubstreamProtocol,
    },
    PeerId, Stream, StreamProtocol,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace};

use super::consts::XSTREAM_PROTOCOL;

/// Возможные события, которые handler может отправить в behaviour
#[derive(Debug)]
pub enum XStreamHandlerEvent {
    /// Установлен новый входящий поток
    IncomingStreamEstablished {
        /// libp2p Stream (сырой поток)
        stream: Stream,
    },
    /// Установлен новый исходящий поток
    OutboundStreamEstablished {
        /// libp2p Stream (сырой поток)
        stream: Stream,
        /// Роль потока
        role: SubstreamRole,
        /// Идентификатор потока
        stream_id: XStreamID,
    },
    /// Произошла ошибка при работе со stream
    StreamError {
        /// Идентификатор потока (если известен)
        stream_id: Option<XStreamID>,
        /// Описание ошибки
        error: String,
    },
    /// Поток XStream закрыт
    StreamClosed {
        /// Идентификатор потока
        stream_id: XStreamID,
    },
}

/// Команды, которые behaviour может отправить в handler
#[derive(Debug)]
pub enum XStreamHandlerIn {
    /// Открыть поток с указанным ID и ролью
    OpenStreamWithRole {
        /// Идентификатор потока
        stream_id: XStreamID,
        /// Роль потока
        role: SubstreamRole,
    },
}

/// Информация о запрошенном потоке
#[derive(Debug, Clone)]
pub struct XStreamOpenInfo {
    pub stream_id: XStreamID,
    pub role: SubstreamRole,
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
    /// Активные потоки
    streams: Vec<XStream>,
    /// Очередь исходящих событий
    outgoing_events: Vec<XStreamHandlerEvent>,
    /// Очередь входящих команд
    pending_commands: Vec<XStreamHandlerIn>,
    /// Peer ID для потоков
    peer_id: PeerId,
    /// Sender for closure notifications
    closure_sender: Option<mpsc::UnboundedSender<(PeerId, XStreamID)>>,

    outgoing_event_sender: mpsc::UnboundedSender<XStreamHandlerEvent>,
    /// Receiver for messages from PendingStreamsManager
    outgoing_event_receiver: mpsc::UnboundedReceiver<XStreamHandlerEvent>,
}

impl XStreamHandler {
    /// Создает новый XStreamHandler
    pub fn new() -> Self {
        // Используем дефолтный PeerId, который будет заменен позже
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            streams: Vec::new(),
            outgoing_events: Vec::new(),
            pending_commands: Vec::new(),
            peer_id: PeerId::random(),
            closure_sender: None,
            outgoing_event_sender: tx,
            outgoing_event_receiver: rx,
        }
    }

    /// Устанавливает peer_id для handler
    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = peer_id;
    }
    /// Sets the closure sender
    pub fn set_closure_sender(&mut self, sender: mpsc::UnboundedSender<(PeerId, XStreamID)>) {
        self.closure_sender = Some(sender);
    }

    /// Получает изменяемый XStream по его ID
    pub fn get_stream_mut(&mut self, stream_id: XStreamID) -> Option<&mut XStream> {
        self.streams.iter_mut().find(|s| s.id == stream_id)
    }

    /// Обрабатывает новый входящий поток - передаем как есть в behaviour
    fn handle_inbound_stream(&mut self, stream: Stream, protocol: StreamProtocol) {
        debug!("Handling new inbound stream");
        // Отправляем поток как есть в behaviour
        let sender = self.outgoing_event_sender.clone();
        tokio::spawn(async move {
            sender
                .send(XStreamHandlerEvent::IncomingStreamEstablished { stream })
                .unwrap();
        });
        //self.outgoing_events
        //    .push(XStreamHandlerEvent::IncomingStreamEstablished { stream });
    }

    /// Обрабатывает новый исходящий поток - передаем с информацией о запросе
    fn handle_outbound_stream(
        &mut self,
        stream: Stream,
        protocol: StreamProtocol,
        info: XStreamOpenInfo,
    ) {
        debug!(
            "Handling new outbound stream with ID: {:?} and role: {:?}",
            info.stream_id, info.role
        );

        // Записываем заголовок перед отправкой потока в behaviour
        let sender = self.outgoing_event_sender.clone();
        tokio::spawn({
            let stream_id = info.stream_id;
            let role = info.role;
            async move {
                // Создаем заголовок
                let header = XStreamHeader::new(stream_id, role);

                // Разделяем поток
                let (mut read, mut write) = AsyncReadExt::split(stream);

                // Записываем заголовок
                if let Err(e) = write_header_to_stream(&mut write, &header).await {
                    error!("Failed to write header to stream: {}", e);
                    // TODO: не смогли открыть один поток, надо уведомить.
                }
                let reunion_stream = read.reunite(write).unwrap();

                // Отправляем поток в behaviour с информацией о роли и ID
                sender
                    .send(XStreamHandlerEvent::OutboundStreamEstablished {
                        stream: reunion_stream,
                        role: info.role,
                        stream_id: info.stream_id,
                    })
                    .unwrap();
            }
        });
    }

    /// Открывает поток с указанным ID и ролью
    fn open_stream_with_role(
        &mut self,
        stream_id: XStreamID,
        role: SubstreamRole,
    ) -> SubstreamProtocol<XStreamProtocol, XStreamOpenInfo> {
        // Создаем протокол с нашим StreamProtocol
        let proto = XStreamProtocol::new(XSTREAM_PROTOCOL.clone());

        // Создаем информацию об открытии
        let info = XStreamOpenInfo { stream_id, role };

        SubstreamProtocol::new(proto, info)
    }

    /// Получает XStream по его ID
    pub fn get_stream(&self, stream_id: XStreamID) -> Option<&XStream> {
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
    type OutboundOpenInfo = XStreamOpenInfo;

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        // Создаем протокол с нашим StreamProtocol
        let proto = XStreamProtocol::new(XSTREAM_PROTOCOL.clone());
        SubstreamProtocol::new(proto, ())
    }

    fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
        match event {
            XStreamHandlerIn::OpenStreamWithRole { stream_id, role } => {
                self.pending_commands
                    .push(XStreamHandlerIn::OpenStreamWithRole { stream_id, role });
            }
        }
    }

    fn connection_keep_alive(&self) -> bool {
        !self.streams.is_empty() || !self.pending_commands.is_empty()
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>,
    > {
        // Обрабатываем исходящие события
        match self.outgoing_event_receiver.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
            }
            Poll::Ready(None) => {
                error!("[POLL] PendingStreams message channel closed unexpectedly");
            }
            Poll::Pending => {
                // No messages from PendingStreamsManager, continue
                trace!("[POLL] No messages from PendingStreamsManager");
            }
        }

        if !self.outgoing_events.is_empty() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                self.outgoing_events.remove(0),
            ));
        }

        // Обрабатываем входящие команды
        if !self.pending_commands.is_empty() {
            match self.pending_commands.remove(0) {
                XStreamHandlerIn::OpenStreamWithRole { stream_id, role } => {
                    // Открываем поток с указанным ID и ролью
                    return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                        protocol: self.open_stream_with_role(stream_id, role),
                    });
                }
            }
        }

        // Нет событий для обработки
        Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<'_, XStreamProtocol, XStreamProtocol, (), XStreamOpenInfo>,
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
                info,
            }) => {
                self.handle_outbound_stream(stream, protocol, info);
            }
            ConnectionEvent::ListenUpgradeError(error) => {
                let sender = self.outgoing_event_sender.clone();
                tokio::spawn(async move {
                    sender
                        .send(XStreamHandlerEvent::StreamError {
                            stream_id: None,
                            error: format!("Listen upgrade error: {:?}", error.error),
                        })
                        .unwrap();
                });
                //self.outgoing_events.push(XStreamHandlerEvent::StreamError {
                //    stream_id: None,
                //    error: format!("Listen upgrade error: {:?}", error.error),
                //});
            }
            ConnectionEvent::DialUpgradeError(error) => {
                let sender = self.outgoing_event_sender.clone();
                tokio::spawn(async move {
                    sender
                        .send(XStreamHandlerEvent::StreamError {
                            stream_id: None,
                            error: format!("Dial upgrade error: {:?}", error.error),
                        })
                        .unwrap();
                });
                //self.outgoing_events.push(XStreamHandlerEvent::StreamError {
                //    stream_id: None,
                //    error: format!("Dial upgrade error: {:?}", error.error),
                //});
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
            let stream_ids: Vec<XStreamID> = self.streams.iter().map(|s| s.id).collect();
            self.streams.clear();

            for stream_id in stream_ids {
                return Poll::Ready(Some(XStreamHandlerEvent::StreamClosed { stream_id }));
            }
        }

        // Иначе сигнализируем о завершении
        Poll::Ready(None)
    }
}

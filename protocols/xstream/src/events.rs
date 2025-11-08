use super::types::XStreamID;
use libp2p::{Multiaddr, PeerId, swarm::ConnectionId};
use tokio::sync::oneshot;

/// Политика принятия решений о входящих апгрейдах
#[derive(Debug, Clone, Copy)]
pub enum IncomingConnectionApprovePolicy {
    /// Автоматически одобрять все входящие апгрейды
    AutoApprove,
    /// Передавать событие в Swarm для обработки
    ApproveViaEvent,
}

/// Решение о входящем апгрейде
#[derive(Debug)]
pub enum InboundUpgradeDecision {
    /// Апгрейд разрешен
    Approved,
    /// Апгрейд отклонен с причиной
    Rejected(String),
}

/// Установленное соединение с информацией о направлении и адресах
#[derive(Debug, Clone)]
pub enum EstablishedConnection {
    /// Входящее соединение
    Inbound {
        /// Удаленный адрес
        remote_addr: Multiaddr,
        /// Локальный адрес
        local_addr: Multiaddr,
    },
    /// Исходящее соединение
    Outbound {
        /// Адрес назначения
        addr: Multiaddr,
    },
}

/// События, генерируемые XStreamNetworkBehaviour
#[derive(Debug)]
pub enum XStreamEvent {
    /// Новый поток XStream установлен
    StreamEstablished {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор потока
        stream_id: XStreamID,
    },
    /// Ошибка при работе с потоком XStream
    StreamError {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор потока
        stream_id: Option<XStreamID>,
        /// Ошибка
        error: String,
    },
    /// Поток XStream закрыт
    StreamClosed {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор потока
        stream_id: XStreamID,
    },
    /// Входящий поток (для обратной совместимости)
    IncomingStream {
        /// Поток XStream
        stream: super::xstream::XStream,
    },
    /// Запрос на принятие решения о входящем апгрейде
    InboundUpgradeRequest {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор соединения
        connection_id: ConnectionId,
        /// Канал для отправки решения
        response_sender: oneshot::Sender<InboundUpgradeDecision>,
    },
}

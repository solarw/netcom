use super::types::XStreamID;
use libp2p::{Multiaddr, PeerId, swarm::ConnectionId};
use std::sync::{Arc, Mutex};
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

/// Отправитель решения об открытии потока
/// Упрощает API для принятия решений о входящих апгрейдах
#[derive(Debug, Clone)]
pub struct StreamOpenDecisionSender {
    /// Внутренний канал для отправки решения
    inner: Arc<Mutex<Option<oneshot::Sender<InboundUpgradeDecision>>>>,
}

impl StreamOpenDecisionSender {
    /// Создает новый отправитель решений
    pub fn new(sender: oneshot::Sender<InboundUpgradeDecision>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(sender))),
        }
    }

    /// Разрешает открытие потока
    pub fn approve(&self) -> Result<(), String> {
        self.send(InboundUpgradeDecision::Approved)
    }

    /// Запрещает открытие потока с указанием причины
    pub fn reject(&self, reason: String) -> Result<(), String> {
        self.send(InboundUpgradeDecision::Rejected(reason))
    }

    /// Отправляет решение через внутренний канал
    fn send(&self, decision: InboundUpgradeDecision) -> Result<(), String> {
        if let Some(sender) = self.inner.lock().unwrap().take() {
            sender.send(decision)
                .map_err(|_| "Failed to send decision - channel closed".to_string())
        } else {
            Err("Decision already sent".to_string())
        }
    }
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
        /// Отправитель решения об открытии потока
        decision_sender: StreamOpenDecisionSender,
    },
}

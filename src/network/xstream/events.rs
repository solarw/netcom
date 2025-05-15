use libp2p::PeerId;
use super::types::XStreamID;

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
}
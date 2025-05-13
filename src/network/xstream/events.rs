use libp2p::PeerId;

/// События, генерируемые XStreamNetworkBehaviour
#[derive(Debug)]
pub enum XStreamEvent {
    /// Новый поток XStream установлен
    StreamEstablished {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор потока
        stream_id: u128,
    },
    /// Ошибка при работе с потоком XStream
    StreamError {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор потока
        stream_id: Option<u128>,
        /// Ошибка
        error: String,
    },
    /// Поток XStream закрыт
    StreamClosed {
        /// Идентификатор пира
        peer_id: PeerId,
        /// Идентификатор потока
        stream_id: u128,
    },
    /// Входящий поток (для обратной совместимости)
    IncomingStream {
        /// Поток XStream
        stream: super::xstream::XStream,
    },
}
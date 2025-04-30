use libp2p::StreamProtocol;

/// Протокол эхо-обмена
pub const PROTOCOL_ID: &str = "/echo/1.0.0";

// Запрос и ответ для нашего протокола (просто векторы байт)
#[derive(Debug, Clone)]
pub struct EchoRequest(pub Vec<u8>);

#[derive(Debug, Clone)]
pub struct EchoResponse(pub Vec<u8>);

// Создаем протокол
pub fn create_protocol() -> StreamProtocol {
    StreamProtocol::new(PROTOCOL_ID)
}

// Создаем конфигурацию для RequestResponse протокола
pub fn create_request_response_config() -> libp2p::request_response::Config {
    libp2p::request_response::Config::default()
}
use super::consts::XSTREAM_PROTOCOL;
use libp2p::StreamProtocol;

/// Возвращает протокол XStream
pub fn xstream_protocol() -> StreamProtocol {
    XSTREAM_PROTOCOL
}

/// Проверяет, является ли протокол протоколом XStream
pub fn is_xstream_protocol(protocol: &StreamProtocol) -> bool {
    protocol == &XSTREAM_PROTOCOL
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::StreamProtocol;

    #[test]
    fn test_xstream_protocol() {
        let protocol = xstream_protocol();
        assert_eq!(protocol, XSTREAM_PROTOCOL);
    }

    #[test]
    fn test_is_xstream_protocol() {
        let protocol = StreamProtocol::new("/xstream/");
        assert!(is_xstream_protocol(&protocol));

        let other_protocol = StreamProtocol::new("/other/");
        assert!(!is_xstream_protocol(&other_protocol));
    }
}
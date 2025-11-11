use futures::{AsyncReadExt, AsyncWriteExt};
use std::{io, time::Duration};
use tokio::time::timeout;

/// Результат handshake
#[derive(Debug)]
pub struct HandshakeResult {
    pub ok: bool,
    pub message: Option<String>,
}

/// Максимальное время ожидания (настраивается)
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Отправляем успешный handshake (OK)
pub async fn write_handshake_ok<W: AsyncWriteExt + Unpin>(mut writer: W) -> io::Result<()> {
    let code_bytes = 0u16.to_be_bytes();
    writer.write_all(&code_bytes).await?;
    writer.flush().await
}

/// Отправляем handshake с ошибкой
pub async fn write_handshake_error<W: AsyncWriteExt + Unpin>(
    mut writer: W,
    message: &str,
) -> io::Result<()> {
    let len = message.len() as u16 + 1; // CODE = len + 1
    let len_bytes = len.to_be_bytes();
    writer.write_all(&len_bytes).await?;
    if !message.is_empty() {
        writer.write_all(message.as_bytes()).await?;
    }
    writer.flush().await
}

/// Читаем handshake с таймаутом
pub async fn read_handshake<R: AsyncReadExt + Unpin>(mut reader: R) -> io::Result<HandshakeResult> {
    let result = timeout(HANDSHAKE_TIMEOUT, async {
        let mut code_buf = [0u8; 2];
        reader.read_exact(&mut code_buf).await?;
        let code = u16::from_be_bytes(code_buf);
        
        if code == 0 {
            Ok(HandshakeResult { ok: true, message: None })
        } else {
            let msg_len = (code - 1) as usize;
            let mut buf = vec![0u8; msg_len];
            if msg_len > 0 {
                reader.read_exact(&mut buf).await?;
            }
            Ok(HandshakeResult {
                ok: false,
                message: Some(String::from_utf8_lossy(&buf).to_string()),
            })
        }
    })
    .await;

    match result {
        Ok(Ok(res)) => Ok(res),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Handshake timeout")),
    }
}

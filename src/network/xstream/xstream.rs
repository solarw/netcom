use std::sync::Arc;
use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use tokio::sync::Mutex;


/// Итератор для генерации уникальных ID

/// Структура для XStream - представляет собой пару потоков для данных и ошибок
#[derive(Debug, Clone)]
pub struct XStream {
    pub stream_main_read: Arc<tokio::sync::Mutex<futures::io::ReadHalf<Stream>>>,
    pub stream_main_write: Arc<tokio::sync::Mutex<futures::io::WriteHalf<Stream>>>,
    pub id: u128,
    pub peer_id: PeerId,
}

impl XStream {
    /// Создает новый XStream из компонентов
    pub fn new(
        id: u128,
        peer_id: PeerId,
        stream_main_read: futures::io::ReadHalf<Stream>,
        stream_main_write: futures::io::WriteHalf<Stream>,
        
    ) -> Self {
        Self {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
            id,
            peer_id,
        }
    }

    /// Читает точное количество байтов из основного потока
    pub async fn read_exact(&self, size: usize) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = vec![0u8; size];
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read_exact(&mut buf).await?;
        Ok(buf)
    }

    /// Читает все данные из основного потока до конца
    pub async fn read_to_end(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    /// Читает доступные данные из основного потока
    pub async fn read(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read(&mut buf).await?;
        Ok(buf)
    }

    /// Записывает все данные в основной поток
    pub async fn write_all(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        let stream_main_write = self.stream_main_write.clone();
        let mut unlocked = stream_main_write.lock().await;
        unlocked.write_all(&buf).await?;
        unlocked.flush().await
    }

    /// Закрывает потоки
    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        let stream_main_write = self.stream_main_write.clone();
        let mut unlocked = stream_main_write.lock().await;
        unlocked.flush().await?;
        unlocked.close().await
    }
}

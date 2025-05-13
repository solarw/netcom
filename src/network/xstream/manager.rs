use crate::network::xstream::pending_streams::PendingStreamsManager;
use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use futures::StreamExt;
use libp2p::{PeerId, Stream, StreamProtocol};
use libp2p_stream::OpenStreamError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, Semaphore};
use crate::network::events::NetworkEvent;

use super::consts::XSTREAM_PROTOCOL;
use super::utils::IdIterator;
use super::xstream::XStream;


/// Менеджер для управления потоками XStream
pub struct StreamManager {
    control: libp2p_stream::Control,
    id_iterator: IdIterator,
    listener: libp2p_stream::IncomingStreams,
    // Используем новый PendingStreamsManager вместо прямого HashMap
    pending_manager: PendingStreamsManager,
    semaphore: Arc<Semaphore>,
    incoming_streams_sender: Arc<mpsc::Sender<XStream>>,
    incoming_streams_receiver: Arc<Mutex<mpsc::Receiver<XStream>>>,
    // Опционально: канал для событий таймаута
    event_tx: Option<mpsc::Sender<NetworkEvent>>,
}

impl StreamManager {
    /// Создает новый менеджер потоков
    pub fn new(mut control: libp2p_stream::Control) -> Self {
        let listener = control.accept(XSTREAM_PROTOCOL).unwrap();
        let (tx, rx) = mpsc::channel(100);
        
        Self {
            control,
            id_iterator: IdIterator::new(),
            listener,
            // Создаем менеджер ожидающих потоков с таймаутом 30 секунд
            pending_manager: PendingStreamsManager::new(Duration::from_secs(30)),
            semaphore: Arc::new(Semaphore::new(1000)),
            incoming_streams_sender: Arc::new(tx),
            incoming_streams_receiver: Arc::new(Mutex::new(rx)),
            event_tx: None,
        }
    }
    
    /// Добавляет канал для отправки событий
    pub fn with_event_channel(mut self, event_tx: mpsc::Sender<NetworkEvent>) -> Self {
        self.event_tx = Some(event_tx);
        self
    }
    
    /// Открывает новый XStream для указанного пира
    pub async fn open_stream(&mut self, peer_id: PeerId) -> Result<XStream, OpenStreamError> {
        // Открываем два потока - основной и для ошибок
        let mut stream_main = self.control.open_stream(peer_id, XSTREAM_PROTOCOL).await?;
        let mut stream_error = self.control.open_stream(peer_id, XSTREAM_PROTOCOL).await?;
        
        // Генерируем уникальный ID для этой пары потоков
        let id = self.id_iterator.next().unwrap();
        
        // Используем методы PendingStreamsManager для создания заголовков
        let main_header = PendingStreamsManager::make_header(id, true);
        let error_header = PendingStreamsManager::make_header(id, false);
        
        // Отправляем заголовки
        stream_main.write_all(&main_header).await?;
        stream_error.write_all(&error_header).await?;

        // Разделяем потоки на чтение/запись
        let (stream_main_read, stream_main_write) = stream_main.split();
        let (stream_error_read, stream_error_write) = stream_error.split();

        // Создаем XStream
        Ok(XStream::new(
            id,
            peer_id,
            stream_main_read,
            stream_main_write,
            stream_error_read,
            stream_error_write,
        ))
    }
    
    /// Обрабатывает входящие потоки
    pub async fn handle_incoming_stream(&mut self) -> Result<(), String> {
        if let Some((peer_id, mut stream)) = self.listener.next().await {
            // Получаем разрешение от семафора
            let _permit = self.semaphore.acquire().await.unwrap();
            
            // Читаем заголовок с использованием PendingStreamsManager
            let header = match PendingStreamsManager::read_header(&mut stream).await {
                Ok(header) => header,
                Err(e) => {
                    // Теперь e имеет тип String, который реализует Send
                    let _ = stream.close().await;
                    return Err(e); // Просто возвращаем ошибку без дополнительного форматирования
                }
            };
            
            // Проверяем, есть ли уже поток с таким ID
            if let Some(mut pending) = self.pending_manager.take_pending_stream(header.id).await { // Добавлено mut
                if pending.is_main == header.is_main {
                    // Получили дубликат того же типа - это ошибка протокола
                    let _ = stream.close().await;
                    let _ = pending.stream.close().await;
                    return Err(format!("Получен дубликат потока с ID: {}", header.id));
                } else {
                    // Получили вторую часть пары
                    let (stream_main, stream_error) = if header.is_main {
                        (stream, pending.stream)
                    } else {
                        (pending.stream, stream)
                    };
                    
                    // Разделяем потоки и создаем XStream
                    let (stream_main_read, stream_main_write) = stream_main.split();
                    let (stream_error_read, stream_error_write) = stream_error.split();
                    
                    let xstream = XStream::new(
                        header.id,
                        peer_id,
                        stream_main_read,
                        stream_main_write,
                        stream_error_read,
                        stream_error_write,
                    );
                    
                    // Отправляем XStream в канал
                    let _ = self.incoming_streams_sender.send(xstream).await;
                }
            } else {
                // Сохраняем поток как ожидающий
                self.pending_manager.add_pending_stream(stream, header.id, header.is_main, peer_id).await;
            }
        }
        
        Ok(())
    }
    
    /// Опрашивает события и возвращает готовый XStream, если доступен
    pub async fn poll(&mut self) -> Option<XStream> {
        let incoming_streams_receiver = self.incoming_streams_receiver.clone();
        let mut receiver = incoming_streams_receiver.lock().await;

        loop {
            tokio::select! {
                result = self.handle_incoming_stream() => {
                    if let Err(e) = result {
                        println!("Ошибка при обработке входящего потока: {}", e);
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    // Очищаем просроченные потоки
                    let event_tx = self.event_tx.clone();
                    self.pending_manager.clean_timed_out_streams(move |id, is_main, peer_id| {
                        println!("Таймаут потока: id={}, is_main={}, peer={}", id, is_main, peer_id);
                        
                        // Отправляем событие таймаута, если настроен event_tx
                        if let Some(_tx) = &event_tx {
                            // Закомментируем условную компиляцию для избежания проблем
                            // #[cfg(feature = "stream_timeout_event")]
                            // let _ = tx.try_send(NetworkEvent::StreamTimeoutEvent {
                            //     id,
                            //     is_main,
                            //     peer_id: Some(peer_id),
                            // });
                        }
                    }).await;
                }
                Some(xstream) = receiver.recv() => {
                    return Some(xstream)
                }
            }
        }
    }
}
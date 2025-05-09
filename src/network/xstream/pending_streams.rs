//! Модуль для управления ожидающими потоками, которые еще не сформировали полные пары.
//! Обеспечивает хранение, сопоставление и очистку по таймауту неполных пар потоков.

use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use futures::AsyncReadExt; // Добавлен импорт AsyncReadExt
use futures::AsyncWriteExt;
use libp2p::{PeerId, Stream};
use std::collections::HashMap; // Добавлен импорт HashMap
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Константа для размера заголовка (16 байт для ID + 1 байт для флага is_main)
pub const HEADER_SIZE: usize = 17;

/// Результат чтения заголовка потока
#[derive(Debug, Clone, Copy)]
pub struct StreamHeader {
    /// Уникальный идентификатор потока/пары потоков
    pub id: u128,
    /// Флаг, указывающий, является ли это основным потоком (true) или потоком ошибок (false)
    pub is_main: bool,
}

/// Структура для хранения информации о ожидающем потоке
pub struct PendingStreamRecord {
    /// Сам поток
    pub stream: Stream,
    /// Уникальный идентификатор потока/пары потоков
    pub id: u128,
    /// Флаг, указывающий, является ли этот поток основным (true) или потоком ошибок (false)
    pub is_main: bool,
    /// Время, когда поток был получен - для отслеживания таймаутов
    pub received_at: Instant,
    /// Идентификатор пира, от которого поступил поток
    pub peer_id: PeerId,
}

impl PendingStreamRecord {
    /// Создает новую запись ожидающего потока
    pub fn new(stream: Stream, id: u128, is_main: bool, peer_id: PeerId) -> Self {
        Self {
            stream,
            id,
            is_main,
            received_at: Instant::now(),
            peer_id,
        }
    }
    
    /// Проверяет, истек ли таймаут для этого потока
    pub fn is_timed_out(&self, timeout_duration: Duration) -> bool {
        Instant::now().duration_since(self.received_at) > timeout_duration
    }
}

/// Менеджер ожидающих потоков - управляет хранением, сопоставлением и очисткой по таймауту
pub struct PendingStreamsManager {
    /// Хранилище ожидающих потоков, индексированное по ID
    pending_streams: Arc<Mutex<HashMap<u128, PendingStreamRecord>>>,
    /// Продолжительность таймаута по умолчанию
    default_timeout: Duration,
}

impl PendingStreamsManager {
    /// Создает новый менеджер ожидающих потоков с указанным таймаутом
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            pending_streams: Arc::new(Mutex::new(HashMap::new())), // Инициализация HashMap::new()
            default_timeout,
        }
    }
    
    /// Создает заголовок потока
    pub fn make_header(id: u128, is_main: bool) -> Vec<u8> {
        let mut header = Vec::with_capacity(HEADER_SIZE);
        header.write_u128::<NetworkEndian>(id).unwrap();
        header.write_u8(is_main as u8).unwrap();
        header
    }
    
    pub async fn read_header(stream: &mut Stream) -> Result<StreamHeader, String> {
        // Заменить тип возвращаемого значения на Result<StreamHeader, String>
        let mut header_bytes = vec![0u8; HEADER_SIZE];
        
        // Обработка ошибок сразу, без захвата переменных
        if let Err(e) = stream.read_exact(&mut header_bytes).await {
            return Err(format!("Ошибка чтения заголовка: {}", e));
        }
        
        let mut cursor = Cursor::new(header_bytes);
        
        // Обработка ошибок сразу
        let id = match cursor.read_u128::<NetworkEndian>() {
            Ok(id) => id,
            Err(e) => return Err(format!("Ошибка при чтении ID из заголовка: {}", e)),
        };
        
        let stream_type_byte = match cursor.read_u8() {
            Ok(byte) => byte,
            Err(e) => return Err(format!("Ошибка при чтении типа потока из заголовка: {}", e)),
        };
        
        let is_main = stream_type_byte != 0;
        
        Ok(StreamHeader { id, is_main })
    }
    
    /// Добавляет новый поток в ожидающие
    pub async fn add_pending_stream(
        &self, 
        stream: Stream, 
        id: u128, 
        is_main: bool, 
        peer_id: PeerId
    ) {
        let mut pending_streams = self.pending_streams.lock().await;
        pending_streams.insert(
            id,
            PendingStreamRecord::new(stream, id, is_main, peer_id),
        );
    }
    
    /// Проверяет, существует ли поток с указанным ID
    pub async fn has_pending_stream(&self, id: u128) -> bool {
        let pending_streams = self.pending_streams.lock().await;
        pending_streams.contains_key(&id)
    }
    
    /// Получает и удаляет поток с указанным ID
    pub async fn take_pending_stream(&self, id: u128) -> Option<PendingStreamRecord> {
        let mut pending_streams = self.pending_streams.lock().await;
        pending_streams.remove(&id)
    }
    
    /// Очищает просроченные потоки
    pub async fn clean_timed_out_streams<F>(&self, on_timeout: F) 
    where
        F: Fn(u128, bool, PeerId) + Send + 'static,
    {
        let mut pending_streams = self.pending_streams.lock().await;
        
        let timed_out_ids: Vec<u128> = pending_streams
            .iter()
            .filter(|(_, record)| record.is_timed_out(self.default_timeout))
            .map(|(id, _)| *id)
            .collect();
        
        for id in timed_out_ids {
            if let Some(mut record) = pending_streams.remove(&id) {  // Изменено на mut record
                // Закрываем поток
                let _ = record.stream.close().await;
                
                // Вызываем callback для обработки таймаута
                on_timeout(id, record.is_main, record.peer_id);
                
                println!("Удален просроченный поток с ID: {}, is_main: {}, peer: {}", 
                        id, record.is_main, record.peer_id);
            }
        }
        
        // Опционально: логируем статистику
        if !pending_streams.is_empty() {
            println!("Текущее количество ожидающих потоков: {}", pending_streams.len());
        }
    }
    
    /// Получает общее количество ожидающих потоков
    pub async fn count(&self) -> usize {
        let pending_streams = self.pending_streams.lock().await;
        pending_streams.len()
    }
    
    /// Получает клон Mutex для прямого доступа (используйте с осторожностью)
    pub fn get_raw_storage(&self) -> Arc<Mutex<HashMap<u128, PendingStreamRecord>>> {
        self.pending_streams.clone()
    }
}

// Структура для представления пары потоков (основной и ошибок)
pub struct StreamPair {
    pub main_stream: Stream,
    pub error_stream: Stream,
    pub id: u128,
    pub peer_id: PeerId,
}
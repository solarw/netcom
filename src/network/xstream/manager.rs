use byteorder::ReadBytesExt;
use byteorder::{NetworkEndian, WriteBytesExt};
use futures::io::{ReadHalf, WriteHalf};
use futures::{prelude::*, SinkExt};
use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::{PeerId, Stream, StreamProtocol};
use libp2p_stream::OpenStreamError;
use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::time::sleep;
const XSTREAM_PROTOCOL: StreamProtocol = StreamProtocol::new("/xstream");

struct IdIterator {
    current: u128,
}
impl IdIterator {
    pub fn new() -> IdIterator {
        return IdIterator { current: 0 };
    }
}

impl Iterator for IdIterator {
    type Item = u128;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        let mut next = 0;
        if current < u128::MAX {
            next = current + 1;
        }
        self.current = next;

        Some(current)
    }
}

// Define a message struct for the channel
enum StreamMessage {
    NewStream(u128, bool, Stream),
    // Add other message types as needed
}

struct PendingStreamRecord {
    stream: Stream,
    id: u128,
    is_main: bool,
}

#[derive(Debug)]
pub struct XStream {
    pub stream_main_read: Arc<tokio::sync::Mutex<ReadHalf<Stream>>>,
    pub stream_main_write: Arc<tokio::sync::Mutex<WriteHalf<Stream>>>,
    pub stream_error_read: Arc<tokio::sync::Mutex<ReadHalf<Stream>>>,
    pub stream_error_write: Arc<tokio::sync::Mutex<WriteHalf<Stream>>>,
    pub id: u128,
    pub peer_id: PeerId,
}

impl XStream {
    pub fn new(
        id: u128,
        peer_id: PeerId,
        stream_main_read: ReadHalf<Stream>,
        stream_main_write: WriteHalf<Stream>,
        stream_error_read: ReadHalf<Stream>,
        stream_error_write: WriteHalf<Stream>,
    ) -> XStream {
        return XStream {
            stream_main_read: Arc::new(Mutex::new(stream_main_read)),
            stream_main_write: Arc::new(Mutex::new(stream_main_write)),
            stream_error_read: Arc::new(Mutex::new(stream_error_read)),
            stream_error_write: Arc::new(Mutex::new(stream_error_write)),
            id: id,
            peer_id: peer_id,
        };
    }
    pub async fn read_exact(&self, size: usize) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = vec![0u8; size];
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read_exact(&mut buf).await?;
        return Ok(buf);
    }

    pub async fn read_to_end(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read_to_end(&mut buf).await?;
        return Ok(buf);
    }

    pub async fn read(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut buf: Vec<u8> = Vec::new();
        let stream_main_read = self.stream_main_read.clone();
        stream_main_read.lock().await.read(&mut buf).await?;
        return Ok(buf);
    }

    pub async fn write_all(&self, buf: Vec<u8>) -> Result<(), std::io::Error> {
        let stream_main_write = self.stream_main_write.clone();
        let mut unlocked = stream_main_write.lock().await;
        match unlocked.write_all(&buf).await {
            Ok(..) => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub async fn close(&mut self) -> Result<(), std::io::Error> {
        let stream_main_write = self.stream_main_write.clone();
        let mut unlocked = stream_main_write.lock().await;
        unlocked.close().await
    }

    /*
    pub fn error_read(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, std::io::Error>> + Send + 'static>> {
        let (err, stream) = self._get_stream(&self.error_stream_read);

        Box::pin(async move {
            if err.is_some() {
                return Err(err.unwrap());
            };
            let mut buf: Vec<u8> = Vec::new();
            match stream.lock().await.read_to_end(&mut buf).await {
                Ok(..) => Ok(buf),
                Err(err) => Err(err),
            }
        })
    }

    pub fn error_write(
        &self,
        buf: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'static>> {
        let (err, stream) = self._get_stream(&self.error_stream_write);

        Box::pin(async move {
            if err.is_some() {
                return Err(err.unwrap());
            };
            let mut s = stream.lock().await;
            match s.write_all(&buf).await {
                Ok(..) => {
                    s.close().await?;
                    Ok(())
                }
                Err(err) => Err(err),
            }
        })
    }
    */
}
pub struct StreamManager {
    control: libp2p_stream::Control,
    negotiated_incoming_streams: Option<String>,
    id_iterator: IdIterator,
    listener: libp2p_stream::IncomingStreams,
    // Use a channel sender instead of a shared HashMap
    pending_streams: Arc<tokio::sync::Mutex<HashMap<u128, PendingStreamRecord>>>,
    semaphore: Arc<Semaphore>,
    incoming_streams_sender: Arc<mpsc::Sender<XStream>>,
    incoming_streams_receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<XStream>>>,
}

impl StreamManager {
    pub fn new(mut control: libp2p_stream::Control) -> StreamManager {
        let listener: libp2p_stream::IncomingStreams =
            control.accept(XSTREAM_PROTOCOL).unwrap();

        let (tx, rx) = mpsc::channel(100);
        return StreamManager {
            control,
            negotiated_incoming_streams: None,
            id_iterator: IdIterator::new(),
            listener,
            pending_streams: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(1000)),
            incoming_streams_sender: Arc::new(tx),
            incoming_streams_receiver: Arc::new(tokio::sync::Mutex::new(rx)),
        };
    }
    pub async fn open_stream(&mut self, peer_id: PeerId) -> Result<XStream, OpenStreamError> {
        let mut stream_main = self.control.open_stream(peer_id, XSTREAM_PROTOCOL).await?;
        let mut stream_error = self.control.open_stream(peer_id, XSTREAM_PROTOCOL).await?;
        let id = self.id_iterator.next().unwrap();
        stream_main.write(make_header(id, true).as_ref()).await?;
        stream_error.write(make_header(id, false).as_ref()).await?;

        let (stream_main_read, stream_main_write) = stream_main.split();
        let (stream_error_read, stream_error_write) = stream_error.split();

        return Ok(XStream::new(
            id,
            peer_id,
            stream_main_read,
            stream_main_write,
            stream_error_read,
            stream_error_write,
        ));
    }
    pub async fn handle_incoming_stream(&mut self) -> Result<(), String> {
        let some = self.listener.next().await;
        if let Some((peer_id, stream)) = some {
            let semaphore = self.semaphore.clone();
            // wait for the slot and listen after
            let permit = semaphore.acquire();
            let pending_streams = self.pending_streams.clone();
            drop(permit);
            // Запускаем отдельную задачу для обработки каждого входящего потока

            let sender = self.incoming_streams_sender.clone();
            tokio::spawn(async move {
                // Получаем разрешение от семафора внутри задачи
                let permit = semaphore.acquire().await.unwrap();

                // Обрабатываем заголовок потока
                let mut stream_clone = stream; // Создаем клон для перемещения в обработчик
                let result = match read_header(&mut stream_clone).await {
                    Ok(value) => Ok(value),
                    Err(e) => Err(format!("Ошибка чтения заголовка: {}", e)),
                };

                match result {
                    Ok((id, is_main)) => {
                        // Добавляем поток в хеш-карту под защитой мьютекса
                        let mut streams = pending_streams.lock().await;

                        println!("Обработан поток с ID: {}, is_main: {}", id, is_main);
                        if let Some(mut pending_stream) = streams.remove(&id) {
                            if pending_stream.is_main == is_main {
                                // bad defined stream. close both
                                let _ = stream_clone.close().await;
                                let _ = pending_stream.stream.close().await;
                            } else {
                                let stream_main_read;
                                let stream_main_write;
                                let stream_error_read;
                                let stream_error_write;
                                if is_main {
                                    (stream_main_read, stream_main_write) = stream_clone.split();
                                    (stream_error_read, stream_error_write) =
                                        pending_stream.stream.split();
                                } else {
                                    (stream_main_read, stream_main_write) =
                                        pending_stream.stream.split();
                                    (stream_error_read, stream_error_write) = stream_clone.split();
                                }

                                let xstream = XStream::new(
                                    id,
                                    peer_id,
                                    stream_main_read,
                                    stream_main_write,
                                    stream_error_read,
                                    stream_error_write,
                                );
                                let _ = sender.send(xstream).await;
                            }
                        } else {
                            streams.insert(
                                id,
                                PendingStreamRecord {
                                    stream: stream_clone,
                                    id: id,
                                    is_main: is_main,
                                },
                            );
                        }

                        // Здесь можно добавить дополнительную логику
                    }
                    Err(err) => {
                        let _ = stream_clone.close().await;
                        println!("{}", err);
                    }
                }
                drop(permit);
            });
        }

        Ok(())
    }

    pub async fn poll(&mut self) -> Option<XStream> {
        let lock = self.incoming_streams_receiver.clone();
        let mut incoming_streams_receiver = lock.lock().await;

        loop {
            // Обрабатываем входящие соединения не блокируя остальные операции
            tokio::select! {
                result = self.handle_incoming_stream() => {
                    if let Err(e) = result {
                        println!("Ошибка при обработке входящего потока: {}", e);
                    }
                }
                _ = sleep(Duration::from_secs(10)) => {
                    // Периодические задания
                    //self.clean_timed_out_connections().await;
                }
                Some(xstream) = incoming_streams_receiver.recv() => {
                    return Some(xstream)
                }
            }
        }
    }
}

fn make_header(id: u128, main: bool) -> Vec<u8> {
    let mut wtr = vec![];
    wtr.write_u128::<NetworkEndian>(id).unwrap();
    wtr.write_u8(main as u8).unwrap();
    return wtr;
}

const HEADER_SIZE: usize = 17;

async fn read_header(stream: &mut Stream) -> Result<(u128, bool), Box<dyn Error>> {
    let mut buf = vec![0u8; HEADER_SIZE];
    stream.read_exact(&mut buf).await?;
    let mut rdr = Cursor::new(buf);
    let request_id = rdr.read_u128::<NetworkEndian>().unwrap();
    let main: bool = rdr.read_u8().unwrap() != 0;

    return Ok((request_id, main));
}

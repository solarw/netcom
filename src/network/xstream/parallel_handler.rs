use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Semaphore};
use tokio::time::timeout;
use anyhow::{Result, anyhow};
use std::future::Future;
use std::pin::Pin;

// Трейт для определения обработчика соединений
// Это позволит создавать различные реализации обработчиков
#[async_trait::async_trait]
pub trait ConnectionProcessor: Send + Sync + 'static {
    // Метод для обработки соединения
    async fn process(&self, stream: Stream) -> Result<()>;

    // Опциональный метод для получения имени обработчика
    fn name(&self) -> &str {
        "DefaultProcessor"
    }
}

// Пример простого обработчика
pub struct EchoProcessor;

#[async_trait::async_trait]
impl ConnectionProcessor for EchoProcessor {
    async fn process(&self, mut stream: TcpStream) -> Result<()> {
        println!("EchoProcessor обрабатывает соединение от: {:?}", stream.peer_addr()?);
        
        // Простой эхо-сервер
        let mut buf = vec![0; 1024];
        loop {
            match stream.try_read(&mut buf) {
                Ok(0) => break, // Соединение закрыто
                Ok(n) => {
                    if let Err(e) = stream.try_write(&buf[0..n]) {
                        eprintln!("Ошибка записи: {}", e);
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Нет данных, ждем
                    tokio::task::yield_now().await;
                }
                Err(e) => {
                    eprintln!("Ошибка чтения: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "EchoProcessor"
    }
}

// Структура для настройки нашего обработчика соединений
pub struct ConnectionHandler<P: ConnectionProcessor> {
    // Семафор для ограничения количества одновременных задач
    limit: Arc<Semaphore>,
    // Канал для отправки соединений на обработку
    sender: mpsc::Sender<TcpStream>,
    // Канал для остановки прослушивания соединений
    shutdown_sender: Option<mpsc::Sender<()>>,
    // Обработчик соединений
    processor: Arc<P>,
    // Таймаут для выполнения задачи
    task_timeout: Duration,
}

impl<P: ConnectionProcessor> ConnectionHandler<P> {
    // Инициализация обработчика с указанным лимитом параллельных задач
    pub fn new(processor: P, max_concurrent_tasks: usize, queue_size: usize, task_timeout: Duration) -> Self {
        // Создаем семафор с указанным количеством разрешений
        let limit = Arc::new(Semaphore::new(max_concurrent_tasks));
        
        // Создаем канал для очереди задач
        let (sender, receiver) = mpsc::channel(queue_size);
        
        // Создаем канал для сигнала завершения
        let (shutdown_sender, mut shutdown_receiver) = mpsc::channel(1);
        
        // Оборачиваем процессор в Arc для безопасного использования в разных потоках
        let processor = Arc::new(processor);
        
        // Запускаем фоновую задачу для обработки входящих соединений
        let worker_limit = limit.clone();
        let worker_sender = sender.clone();
        let worker_processor = processor.clone();
        let worker_timeout = task_timeout;
        
        tokio::spawn(async move {
            tokio::select! {
                _ = Self::process_queue(receiver, worker_limit, worker_processor, worker_timeout) => {
                    println!("Обработчик очереди завершил работу");
                }
                _ = shutdown_receiver.recv() => {
                    println!("Получен сигнал завершения, останавливаем обработчик очереди");
                    // Закрываем канал отправки, что приведет к завершению process_queue
                    drop(worker_sender);
                }
            }
        });
        
        ConnectionHandler { 
            limit, 
            sender,
            shutdown_sender: Some(shutdown_sender),
            processor,
            task_timeout,
        }
    }
    
    // Метод для обработки входящих соединений из очереди
    async fn process_queue(
        mut receiver: mpsc::Receiver<TcpStream>,
        limit: Arc<Semaphore>,
        processor: Arc<P>,
        task_timeout: Duration,
    ) {
        // Постоянно слушаем очередь на новые соединения
        while let Some(stream) = receiver.recv().await {
            // Создаем копию счетчика для этого соединения
            let permit_limit = limit.clone();
            let conn_processor = processor.clone();
            let timeout_duration = task_timeout;
            
            // Запускаем обработку в отдельной задаче
            tokio::spawn(async move {
                // Пытаемся получить разрешение от семафора
                let permit = match permit_limit.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        eprintln!("Ошибка получения разрешения: {}", e);
                        return;
                    }
                };
                
                // Обрабатываем соединение с ограничением по времени
                if let Err(e) = Self::handle_connection_with_timeout(stream, conn_processor, timeout_duration).await {
                    eprintln!("Ошибка обработки соединения: {}", e);
                }
                
                // Разрешение автоматически освобождается, когда permit выходит из области видимости
                drop(permit);
            });
        }
    }
    
    // Метод для обработки одного соединения с таймаутом
    async fn handle_connection_with_timeout(
        stream: TcpStream, 
        processor: Arc<P>,
        timeout_duration: Duration
    ) -> Result<()> {
        match timeout(timeout_duration, processor.process(stream)).await {
            Ok(result) => result,
            Err(_) => Err(anyhow!("Таймаут обработки соединения превышен ({:?})", timeout_duration)),
        }
    }
    
    // Метод для добавления нового соединения в очередь
    pub async fn accept_connection(&self, stream: TcpStream) -> Result<()> {
        // Отправляем соединение в очередь обработки
        self.sender.send(stream).await
            .map_err(|_| anyhow!("Очередь соединений заполнена или закрыта"))
    }
    
    // Метод для корректного закрытия обработчика соединений
    pub async fn shutdown(&mut self) {
        println!("Начинаем процесс завершения работы обработчика соединений...");
        
        // Отправляем сигнал завершения, если канал еще не закрыт
        if let Some(sender) = self.shutdown_sender.take() {
            if let Err(e) = sender.send(()).await {
                eprintln!("Ошибка при отправке сигнала остановки: {}", e);
            }
        }
        
        // Закрываем канал отправки соединений, что приведет к завершению процесса обработки очереди
        // когда все текущие соединения будут обработаны
        drop(self.sender.clone());
        
        println!("Обработчик соединений успешно остановлен");
    }
    
    // Получить имя используемого процессора
    pub fn processor_name(&self) -> &str {
        self.processor.name()
    }
}

// Пример пользовательского обработчика соединений
pub struct CustomProcessor {
    name: String,
}

impl CustomProcessor {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait::async_trait]
impl ConnectionProcessor for CustomProcessor {
    async fn process(&self, stream: TcpStream) -> Result<()> {
        println!("{} обрабатывает соединение от: {:?}", self.name, stream.peer_addr()?);
        
        // Пример собственной логики обработки
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        println!("{} завершил обработку соединения", self.name);
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

// Пример использования обработчика соединений с возможностью корректного завершения
#[tokio::main]
async fn main() -> Result<()> {
    // Создаем пользовательский обработчик
    let processor = CustomProcessor::new("МойПроцессор");
    
    // Настраиваем параметры обработчика соединений
    let max_concurrent_tasks = 10;
    let queue_size = 100;
    let task_timeout = Duration::from_secs(10);
    
    // Создаем обработчик соединений с нашим пользовательским процессором
    let mut handler = ConnectionHandler::new(
        processor,
        max_concurrent_tasks, 
        queue_size,
        task_timeout
    );
    
    println!("Используется процессор: {}", handler.processor_name());
    
    // Привязываемся к порту и начинаем принимать соединения
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Сервер запущен на: 127.0.0.1:8080");
    
    // Создаем канал для перехвата сигнала завершения (например, Ctrl+C)
    let (shutdown_send, mut shutdown_recv) = mpsc::channel::<()>(1);
    
    // Настраиваем обработчик Ctrl+C
    let shutdown_sender = shutdown_send.clone();
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            println!("Получен сигнал Ctrl+C, начинаем завершение работы...");
            let _ = shutdown_sender.send(()).await;
        }
    });
    
    // Основной цикл обработки соединений
    tokio::select! {
        result = async {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        println!("Новое соединение: {}", addr);
                        
                        // Добавляем соединение в очередь на обработку
                        if let Err(e) = handler.accept_connection(stream).await {
                            eprintln!("Не удалось поставить соединение в очередь: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Ошибка принятия соединения: {}", e);
                    }
                }
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        } => {
            result?;
        }
        _ = shutdown_recv.recv() => {
            println!("Основной поток получил сигнал завершения. Закрываем сервер...");
        }
    }
    
    // Корректно завершаем работу обработчика соединений
    handler.shutdown().await;
    
    println!("Сервер успешно остановлен");
    Ok(())
}
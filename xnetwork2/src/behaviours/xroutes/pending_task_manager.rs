//! PendingTaskManager - централизованное управление асинхронными задачами с таймаутом
//! 
//! Позволяет создавать задачи с таймаутом и устанавливать результат или ошибку
//! Автоматически отменяет таймауты при установке результата и очищает устаревшие задачи

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};
use tokio::task::{JoinHandle, AbortHandle};
use tokio::sync::oneshot;
use std::fmt;

/// Ошибки PendingTaskManager
#[derive(Debug)]
pub enum PendingTaskError {
    /// Задача уже завершена (таймаут или результат уже установлен)
    TaskAlreadyCompleted,
    /// Задача не найдена
    TaskNotFound,
    /// Ошибка отправки результата
    SendError(String),
}

impl std::error::Error for PendingTaskError {}

impl fmt::Display for PendingTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PendingTaskError::TaskAlreadyCompleted => write!(f, "Task already completed"),
            PendingTaskError::TaskNotFound => write!(f, "Task not found"),
            PendingTaskError::SendError(msg) => write!(f, "Send error: {}", msg),
        }
    }
}

/// Ошибка таймаута задачи
#[derive(Debug, Clone)]
pub struct TaskTimeoutError;

impl std::error::Error for TaskTimeoutError {}

impl fmt::Display for TaskTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task timeout")
    }
}

/// Структура для хранения информации о задаче
struct PendingTask<T, O, E> {
    task_id: T,
    due_to: Instant,
    result_tx: oneshot::Sender<Result<O, E>>,
    handler: AbortHandle,
}

/// Централизованный менеджер асинхронных задач с таймаутом
pub struct PendingTaskManager<T, O, E> {
    tasks: Arc<Mutex<HashMap<T, PendingTask<T, O, E>>>>,
}

// Убираем реализацию Default, так как она требует, чтобы T и R тоже были Default
// Вместо этого используем явный вызов new()

impl<T, O, E> PendingTaskManager<T, O, E> 
where 
    T: Eq + std::hash::Hash + Clone + Send + 'static,
    O: Send + 'static,
    E: From<TaskTimeoutError> + Send + 'static,
{
    /// Создает новый PendingTaskManager
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Добавляет задачу с таймаутом
    /// 
    /// # Аргументы
    /// - `task_id`: уникальный идентификатор задачи
    /// - `timeout`: время таймаута
    /// - `result_tx`: канал для отправки результата
    /// 
    /// # Примечание
    /// При срабатывании таймаута отправляется ошибка TaskTimeoutError через канал.
    pub fn add_pending_task(
        &self,
        task_id: T,
        timeout: Duration,
        result_tx: oneshot::Sender<Result<O, E>>,
    ) {
        let due_to = Instant::now() + timeout;
        let tasks = Arc::clone(&self.tasks);
        let task_id_clone = task_id.clone();

        // Создаем фоновую задачу для таймаута
        let handler = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            
            let mut tasks = tasks.lock().unwrap();
            if let Some(pending_task) = tasks.remove(&task_id_clone) {
                // При таймауте отправляем ошибку TaskTimeoutError
            let _ = pending_task.result_tx.send(Err(TaskTimeoutError.into()));
            }
        }).abort_handle();

        let pending_task = PendingTask {
            task_id: task_id.clone(),
            due_to,
            result_tx,
            handler,
        };

        self.tasks.lock().unwrap().insert(task_id, pending_task);
    }

    /// Устанавливает успешный результат задачи и отменяет таймаут
    /// 
    /// # Аргументы
    /// - `task_id`: идентификатор задачи
    /// - `result`: успешный результат для отправки
    /// 
    /// # Возвращает
    /// - `Ok(true)`: если результат успешно установлен
    /// - `Ok(false)`: если задача не найдена
    /// - `Err`: если произошла ошибка отправки
    pub fn set_task_result(&self, task_id: &T, result: O) -> Result<bool, PendingTaskError> {
        let mut tasks = self.tasks.lock().unwrap();
        
        if let Some(pending_task) = tasks.remove(task_id) {
            // Отменяем таймаут
            pending_task.handler.abort();
            
            // Отправляем успешный результат
            match pending_task.result_tx.send(Ok(result)) {
                Ok(()) => Ok(true),
                Err(_) => Err(PendingTaskError::TaskAlreadyCompleted),
            }
        } else {
            Ok(false)
        }
    }

    /// Очищает устаревшие задачи (таймаут которых уже истек)
    /// 
    /// Возвращает количество удаленных задач
    pub fn cleanup_expired(&self) -> usize {
        let now = Instant::now();
        let mut tasks = self.tasks.lock().unwrap();
        
        let initial_len = tasks.len();
        tasks.retain(|_, pending_task| {
            if now >= pending_task.due_to {
                // Задача истекла - отменяем таймер
                pending_task.handler.abort();
                false
            } else {
                true
            }
        });
        
        initial_len - tasks.len()
    }

    /// Возвращает количество активных задач
    pub fn active_tasks_count(&self) -> usize {
        self.tasks.lock().unwrap().len()
    }

    /// Очищает все задачи (отменяет все таймауты)
    pub fn clear_all(&self) {
        let mut tasks = self.tasks.lock().unwrap();
        for (_, pending_task) in tasks.drain() {
            pending_task.handler.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    // Реализация From<TaskTimeoutError> для String для использования в тестах
    impl From<TaskTimeoutError> for String {
        fn from(_: TaskTimeoutError) -> Self {
            "Task timeout".to_string()
        }
    }

    #[tokio::test]
    async fn test_successful_completion_before_timeout() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        manager.add_pending_task(1, Duration::from_secs(10), tx);
        
        // Устанавливаем успешный результат до таймаута
        let result = manager.set_task_result(&1, "success".to_string());
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Проверяем, что результат получен
        let received = rx.await.unwrap();
        assert_eq!(received, Ok("success".to_string()));
        
        // Проверяем, что задача удалена
        assert_eq!(manager.active_tasks_count(), 0);
    }

    #[tokio::test]
    async fn test_timeout_triggered() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        manager.add_pending_task(1, Duration::from_millis(100), tx);
        
        // Ждем таймаут
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Проверяем, что задача удалена после таймаута
        assert_eq!(manager.active_tasks_count(), 0);
        
        // Проверяем, что получена ошибка таймаута
        let result = rx.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task timeout");
    }

    #[tokio::test]
    async fn test_double_result_set() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        manager.add_pending_task(1, Duration::from_secs(10), tx);
        
        // Первая установка результата - успешно
        let result1 = manager.set_task_result(&1, "first".to_string());
        assert!(result1.is_ok());
        assert!(result1.unwrap());
        
        // Вторая установка результата - должна вернуть ошибку
        let result2 = manager.set_task_result(&1, "second".to_string());
        assert!(matches!(result2, Ok(false))); // Задача не найдена
        
        // Проверяем, что получен первый результат
        let received = rx.await.unwrap();
        assert_eq!(received, Ok("first".to_string()));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        
        // Добавляем задачу с очень коротким таймаутом
        let (tx1, _) = oneshot::channel();
        manager.add_pending_task(1, Duration::from_millis(50), tx1);
        
        // Добавляем задачу с длинным таймаутом
        let (tx2, _) = oneshot::channel();
        manager.add_pending_task(2, Duration::from_secs(10), tx2);
        
        // Проверяем, что обе задачи активны
        assert_eq!(manager.active_tasks_count(), 2);
        
        // Ждем, пока первая задача истечет
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Очищаем устаревшие задачи
        let cleaned = manager.cleanup_expired();
        // После таймаута задача уже удалена автоматически, поэтому очистка не найдет устаревших
        assert_eq!(cleaned, 0);
        // Остается только вторая задача
        assert_eq!(manager.active_tasks_count(), 1);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let manager: StdArc<PendingTaskManager<u32, u32, String>> = StdArc::new(PendingTaskManager::new());
        
        let mut handles = vec![];
        
        // Создаем 4 задачи параллельно - 2 успешные, 2 таймаутные
        for i in 0..4 {
            let manager = manager.clone();
            let handle = tokio::spawn(async move {
                let (tx, rx) = oneshot::channel();
                manager.add_pending_task(i, Duration::from_millis(500), tx); // Короткий таймаут для теста
                
                // Четные задачи завершаем успешно, нечетные оставляем на таймаут
                if i % 2 == 0 {
                    // Успешное завершение
                    let result = manager.set_task_result(&i, i as u32);
                    assert!(result.is_ok());
                    assert!(result.unwrap());
                    
                    // Проверяем, что результат получен
                    let received = rx.await.unwrap();
                    assert_eq!(received, Ok(i as u32));
                } else {
                    // Таймаутная задача - ждем пока сработает таймаут
                    let result = rx.await.unwrap();
                    assert!(result.is_err());
                    assert_eq!(result.unwrap_err(), "Task timeout");
                }
            });
            handles.push(handle);
        }
        
        // Ждем завершения всех задач
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Даем время таймаутным задачам завершиться
        tokio::time::sleep(Duration::from_millis(600)).await;
        
        // Очищаем все оставшиеся задачи
        let cleaned = manager.cleanup_expired();
        // После таймаута задачи уже удалены автоматически, поэтому очистка не найдет устаревших
        assert_eq!(cleaned, 0);
        
        // Проверяем, что все задачи завершены
        assert_eq!(manager.active_tasks_count(), 0);
    }

    #[tokio::test]
    async fn test_task_not_found() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        
        // Пытаемся установить результат для несуществующей задачи
        let result = manager.set_task_result(&999, "not found".to_string());
        assert!(matches!(result, Ok(false)));
    }

    #[tokio::test]
    async fn test_clear_all() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        
        // Добавляем несколько задач
        for i in 0..5 {
            let (tx, _) = oneshot::channel();
            manager.add_pending_task(i, Duration::from_secs(10), tx);
        }
        
        assert_eq!(manager.active_tasks_count(), 5);
        
        // Очищаем все
        manager.clear_all();
        
        assert_eq!(manager.active_tasks_count(), 0);
    }

    // Тесты на граничные случаи для надежности

    #[tokio::test]
    async fn test_zero_timeout() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        // Добавляем задачу с нулевым таймаутом
        manager.add_pending_task(1, Duration::from_millis(0), tx);
        
        // Даем время для выполнения таймаута
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Проверяем, что задача удалена после таймаута
        assert_eq!(manager.active_tasks_count(), 0);
        
        // Проверяем, что получена ошибка таймаута
        let result = rx.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task timeout");
    }

    #[tokio::test]
    async fn test_very_short_timeout() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        // Добавляем задачу с очень коротким таймаутом
        manager.add_pending_task(1, Duration::from_micros(100), tx); // 100 микросекунд
        
        // Даем время для выполнения таймаута
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        // Проверяем, что задача удалена после таймаута
        assert_eq!(manager.active_tasks_count(), 0);
        
        // Проверяем, что получена ошибка таймаута
        let result = rx.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task timeout");
    }

    #[tokio::test]
    async fn test_task_cancellation_before_timeout() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        manager.add_pending_task(1, Duration::from_secs(10), tx);
        
        // Отменяем задачу через clear_all до таймаута
        manager.clear_all();
        
        // Проверяем, что задача удалена
        assert_eq!(manager.active_tasks_count(), 0);
        
        // Проверяем, что канал закрыт (задача отменена)
        assert!(rx.await.is_err());
    }

    #[tokio::test]
    async fn test_reuse_task_id() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        
        // Первая задача с task_id = 1
        let (tx1, rx1) = oneshot::channel();
        manager.add_pending_task(1, Duration::from_secs(10), tx1);
        
        // Устанавливаем результат для первой задачи
        let result1 = manager.set_task_result(&1, "first".to_string());
        assert!(result1.is_ok());
        assert!(result1.unwrap());
        
        // Проверяем, что первая задача завершена
        let received1 = rx1.await.unwrap();
        assert_eq!(received1, Ok("first".to_string()));
        
        // Вторая задача с тем же task_id = 1
        let (tx2, rx2) = oneshot::channel();
        manager.add_pending_task(1, Duration::from_secs(10), tx2);
        
        // Устанавливаем результат для второй задачи
        let result2 = manager.set_task_result(&1, "second".to_string());
        assert!(result2.is_ok());
        assert!(result2.unwrap());
        
        // Проверяем, что вторая задача завершена
        let received2 = rx2.await.unwrap();
        assert_eq!(received2, Ok("second".to_string()));
        
        // Проверяем, что все задачи удалены
        assert_eq!(manager.active_tasks_count(), 0);
    }

    #[tokio::test]
    async fn test_high_concurrency() {
        let manager: StdArc<PendingTaskManager<u32, u32, String>> = StdArc::new(PendingTaskManager::new());
        
        let mut handles = vec![];
        let num_tasks = 100;
        
        // Создаем много задач параллельно
        for i in 0..num_tasks {
            let manager = manager.clone();
            let handle = tokio::spawn(async move {
                let (tx, rx) = oneshot::channel();
                manager.add_pending_task(i, Duration::from_millis(500), tx);
                
                // Половина задач завершается успешно, половина таймаутит
                if i % 2 == 0 {
                    let _ = manager.set_task_result(&i, i as u32);
                    let _ = rx.await; // Игнорируем результат
                } else {
                    let _ = rx.await; // Ждем таймаут
                }
            });
            handles.push(handle);
        }
        
        // Ждем завершения всех задач
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Даем время таймаутным задачам завершиться
        tokio::time::sleep(Duration::from_millis(600)).await;
        
        // Очищаем все оставшиеся задачи
        manager.cleanup_expired();
        
        // Проверяем, что все задачи завершены
        assert_eq!(manager.active_tasks_count(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_multiple_expired() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        
        // Добавляем несколько задач с короткими таймаутами
        for i in 0..5 {
            let (tx, _) = oneshot::channel();
            manager.add_pending_task(i, Duration::from_millis(50), tx);
        }
        
        // Добавляем задачу с длинным таймаутом
        let (tx_long, _) = oneshot::channel();
        manager.add_pending_task(999, Duration::from_secs(10), tx_long);
        
        // Проверяем, что все задачи активны
        assert_eq!(manager.active_tasks_count(), 6);
        
        // Ждем, пока короткие задачи истекут
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Очищаем устаревшие задачи
        let cleaned = manager.cleanup_expired();
        // После таймаута задачи уже удалены автоматически, поэтому очистка не найдет устаревших
        assert_eq!(cleaned, 0);
        
        // Остается только задача с длинным таймаутом
        assert_eq!(manager.active_tasks_count(), 1);
    }

    #[tokio::test]
    async fn test_set_result_after_timeout() {
        let manager: PendingTaskManager<u32, String, String> = PendingTaskManager::new();
        let (tx, rx) = oneshot::channel();
        
        manager.add_pending_task(1, Duration::from_millis(50), tx);
        
        // Ждем таймаут
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Пытаемся установить результат после таймаута
        let result = manager.set_task_result(&1, "too late".to_string());
        // Задача уже удалена таймаутом, поэтому не найдена
        assert!(matches!(result, Ok(false)));
        
        // Проверяем, что получена ошибка таймаута
        let result = rx.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task timeout");
        
        // Проверяем, что задача удалена
        assert_eq!(manager.active_tasks_count(), 0);
    }
}

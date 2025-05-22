// xstream_error.rs
// Error types and handling for XStream operations

use std::fmt;
use std::io;

/// XStream-специфичная ошибка, переданная от сервера
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XStreamError {
    /// Данные ошибки от сервера
    pub data: Vec<u8>,
    /// Человеко-читаемое сообщение (если данные можно интерпретировать как UTF-8)
    pub message: Option<String>,
}

impl XStreamError {
    /// Создает новую XStreamError из данных
    pub fn new(data: Vec<u8>) -> Self {
        let message = if !data.is_empty() {
            String::from_utf8(data.clone()).ok()
        } else {
            None
        };

        Self { data, message }
    }

    /// Создает XStreamError из строкового сообщения
    pub fn from_message(message: String) -> Self {
        Self {
            data: message.as_bytes().to_vec(),
            message: Some(message),
        }
    }

    /// Возвращает сырые данные ошибки
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Возвращает человеко-читаемое сообщение, если доступно
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Пытается интерпретировать данные как UTF-8 строку
    pub fn as_string(&self) -> Option<String> {
        String::from_utf8(self.data.clone()).ok()
    }

    /// Проверяет, пустая ли ошибка
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Возвращает размер данных ошибки
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl fmt::Display for XStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref message) = self.message {
            write!(f, "XStream error: {}", message)
        } else if !self.data.is_empty() {
            write!(f, "XStream error: {} bytes of binary data", self.data.len())
        } else {
            write!(f, "XStream error: empty")
        }
    }
}

impl std::error::Error for XStreamError {}

/// Ошибка чтения с частично прочитанными данными
#[derive(Debug, Clone)]
pub struct ErrorOnRead {
    /// Частично прочитанные данные перед получением ошибки
    pub partial_data: Vec<u8>,
    /// Ошибка, которая прервала чтение
    pub error: ReadError,
}

/// Тип ошибки, которая может произойти при чтении
#[derive(Debug, Clone)]
pub enum ReadError {
    /// Стандартная IO ошибка Rust
    Io(IoErrorWrapper),
    /// XStream ошибка от сервера
    XStream(XStreamError),
}

/// Обертка для io::Error чтобы сделать её Clone
#[derive(Debug, Clone)]
pub struct IoErrorWrapper {
    kind: io::ErrorKind,
    message: String,
}

impl IoErrorWrapper {
    pub fn new(error: io::Error) -> Self {
        Self {
            kind: error.kind(),
            message: error.to_string(),
        }
    }

    pub fn kind(&self) -> io::ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Конвертирует обратно в io::Error
    pub fn to_io_error(&self) -> io::Error {
        io::Error::new(self.kind, self.message.clone())
    }
}

impl fmt::Display for IoErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<io::Error> for IoErrorWrapper {
    fn from(error: io::Error) -> Self {
        Self::new(error)
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        ReadError::Io(IoErrorWrapper::new(error))
    }
}

impl From<XStreamError> for ReadError {
    fn from(error: XStreamError) -> Self {
        ReadError::XStream(error)
    }
}

impl From<Vec<u8>> for XStreamError {
    fn from(data: Vec<u8>) -> Self {
        XStreamError::new(data)
    }
}

impl ErrorOnRead {
    /// Создает новую ErrorOnRead
    pub fn new(partial_data: Vec<u8>, error: ReadError) -> Self {
        Self {
            partial_data,
            error,
        }
    }

    /// Compatibility method for tests - returns ErrorKind if this is an IO error
    pub fn kind(&self) -> io::ErrorKind {
        match &self.error {
            ReadError::Io(io_wrapper) => io_wrapper.kind(),
            ReadError::XStream(_) => io::ErrorKind::Other,
        }
    }

    /// Создает ErrorOnRead с IO ошибкой
    pub fn from_io_error(partial_data: Vec<u8>, error: io::Error) -> Self {
        Self {
            partial_data,
            error: ReadError::Io(IoErrorWrapper::new(error)),
        }
    }

    /// Создает ErrorOnRead с XStream ошибкой
    pub fn from_xstream_error(partial_data: Vec<u8>, error: XStreamError) -> Self {
        Self {
            partial_data,
            error: ReadError::XStream(error),
        }
    }

    /// Создает ErrorOnRead с XStream ошибкой из данных
    pub fn from_xstream_data(partial_data: Vec<u8>, error_data: Vec<u8>) -> Self {
        Self {
            partial_data,
            error: ReadError::XStream(XStreamError::new(error_data)),
        }
    }

    /// Возвращает частично прочитанные данные
    pub fn partial_data(&self) -> &[u8] {
        &self.partial_data
    }

    /// Возвращает ссылку на ошибку
    pub fn error(&self) -> &ReadError {
        &self.error
    }

    /// Проверяет, является ли ошибка IO ошибкой
    pub fn is_io_error(&self) -> bool {
        matches!(self.error, ReadError::Io(_))
    }

    /// Проверяет, является ли ошибка XStream ошибкой
    pub fn is_xstream_error(&self) -> bool {
        matches!(self.error, ReadError::XStream(_))
    }

    /// Возвращает IO ошибку, если это IO ошибка
    pub fn as_io_error(&self) -> Option<&IoErrorWrapper> {
        match &self.error {
            ReadError::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Возвращает XStream ошибку, если это XStream ошибка
    pub fn as_xstream_error(&self) -> Option<&XStreamError> {
        match &self.error {
            ReadError::XStream(err) => Some(err),
            _ => None,
        }
    }

    /// Извлекает частично прочитанные данные, потребляя структуру
    pub fn into_partial_data(self) -> Vec<u8> {
        self.partial_data
    }

    /// Извлекает ошибку, потребляя структуру
    pub fn into_error(self) -> ReadError {
        self.error
    }

    /// Разбирает на составные части
    pub fn into_parts(self) -> (Vec<u8>, ReadError) {
        (self.partial_data, self.error)
    }

    /// Проверяет, есть ли частично прочитанные данные
    pub fn has_partial_data(&self) -> bool {
        !self.partial_data.is_empty()
    }

    /// Возвращает размер частично прочитанных данных
    pub fn partial_data_len(&self) -> usize {
        self.partial_data.len()
    }

    /// Создает ErrorOnRead только с ошибкой (без частичных данных)
    pub fn error_only(error: ReadError) -> Self {
        Self {
            partial_data: Vec::new(),
            error,
        }
    }

    /// Создает ErrorOnRead только с IO ошибкой (без частичных данных)
    pub fn io_error_only(error: io::Error) -> Self {
        Self::from_io_error(Vec::new(), error)
    }

    /// Создает ErrorOnRead только с XStream ошибкой (без частичных данных)
    pub fn xstream_error_only(error: XStreamError) -> Self {
        Self::from_xstream_error(Vec::new(), error)
    }

    /// Compatibility method: converts ErrorOnRead to io::Error for legacy code
    pub fn to_io_error(self) -> io::Error {
        match self.error {
            ReadError::Io(io_wrapper) => io_wrapper.to_io_error(),
            ReadError::XStream(xs_error) => {
                io::Error::new(io::ErrorKind::Other, format!("XStream error: {}", xs_error))
            }
        }
    }

    /// Compatibility method: creates ErrorOnRead from io::Error (for test compatibility)
    pub fn from_std_io_error(error: io::Error) -> Self {
        Self::io_error_only(error)
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(err) => write!(f, "IO error: {}", err),
            ReadError::XStream(err) => write!(f, "XStream error: {}", err),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReadError::Io(_) => None, // IoErrorWrapper не implement Error
            ReadError::XStream(err) => Some(err),
        }
    }
}

impl fmt::Display for ErrorOnRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.has_partial_data() {
            write!(
                f,
                "Error occurred after reading {} bytes: {}",
                self.partial_data_len(),
                self.error
            )
        } else {
            write!(f, "Error occurred before reading any data: {}", self.error)
        }
    }
}

impl std::error::Error for ErrorOnRead {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Результат операции чтения XStream
pub type XStreamReadResult<T> = Result<T, ErrorOnRead>;

// For backward compatibility with existing tests
impl From<io::Error> for ErrorOnRead {
    fn from(error: io::Error) -> Self {
        ErrorOnRead::io_error_only(error)
    }
}

/// Утилиты для работы с ошибками XStream
pub mod utils {
    use super::*;

    /// Конвертирует io::Error в ErrorOnRead без частичных данных
    pub fn io_error_to_error_on_read(error: io::Error) -> ErrorOnRead {
        ErrorOnRead::io_error_only(error)
    }

    /// Конвертирует данные ошибки XStream в ErrorOnRead без частичных данных
    pub fn xstream_data_to_error_on_read(error_data: Vec<u8>) -> ErrorOnRead {
        ErrorOnRead::xstream_error_only(XStreamError::new(error_data))
    }

    /// Создает ErrorOnRead из стандартного Result<T, io::Error> с частичными данными
    pub fn result_with_partial_data<T>(
        result: Result<T, io::Error>,
        partial_data: Vec<u8>,
    ) -> Result<T, ErrorOnRead> {
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(ErrorOnRead::from_io_error(partial_data, err)),
        }
    }

    /// Проверяет, является ли ошибка критической (требует закрытия соединения)
    pub fn is_critical_error(error: &ReadError) -> bool {
        match error {
            ReadError::Io(io_err) => matches!(
                io_err.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::NotConnected
            ),
            ReadError::XStream(_) => false, // XStream ошибки не критические для соединения
        }
    }

    /// Получает человеко-читаемое описание ошибки
    pub fn error_description(error: &ReadError) -> String {
        match error {
            ReadError::Io(io_err) => format!("IO error ({}): {}", io_err.kind(), io_err.message()),
            ReadError::XStream(xs_err) => {
                if let Some(message) = xs_err.message() {
                    format!("XStream error: {}", message)
                } else {
                    format!("XStream error: {} bytes", xs_err.len())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xstream_error_creation() {
        // Тест создания из данных
        let data = b"Error message".to_vec();
        let error = XStreamError::new(data.clone());
        assert_eq!(error.data(), &data);
        assert_eq!(error.message(), Some("Error message"));

        // Тест создания из строки
        let error2 = XStreamError::from_message("Test error".to_string());
        assert_eq!(error2.as_string(), Some("Test error".to_string()));
    }

    #[test]
    fn test_error_on_read_creation() {
        let partial_data = b"partial".to_vec();
        let io_error = io::Error::new(io::ErrorKind::BrokenPipe, "Pipe broken");
        
        let error_on_read = ErrorOnRead::from_io_error(partial_data.clone(), io_error);
        
        assert_eq!(error_on_read.partial_data(), &partial_data);
        assert!(error_on_read.is_io_error());
        assert!(!error_on_read.is_xstream_error());
        assert!(error_on_read.has_partial_data());
        assert_eq!(error_on_read.partial_data_len(), 7);
    }

    #[test]
    fn test_io_error_wrapper() {
        let original = io::Error::new(io::ErrorKind::TimedOut, "Operation timed out");
        let wrapper = IoErrorWrapper::new(original);
        
        assert_eq!(wrapper.kind(), io::ErrorKind::TimedOut);
        assert!(wrapper.message().contains("timed out"));
        
        let restored = wrapper.to_io_error();
        assert_eq!(restored.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn test_error_on_read_compatibility() {
        // Test kind() method compatibility
        let io_error = io::Error::new(io::ErrorKind::BrokenPipe, "Pipe broken");
        let error_on_read = ErrorOnRead::from(io_error);
        assert_eq!(error_on_read.kind(), io::ErrorKind::BrokenPipe);
        
        // Test XStream error returns Other kind
        let xs_error = XStreamError::new(b"test".to_vec());
        let xs_error_on_read = ErrorOnRead::xstream_error_only(xs_error);
        assert_eq!(xs_error_on_read.kind(), io::ErrorKind::Other);
    }
    
    #[test]
    fn test_error_conversion() {
        let original = io::Error::new(io::ErrorKind::TimedOut, "Timeout");
        let error_on_read = ErrorOnRead::from(original);
        let converted_back = error_on_read.to_io_error();
        assert_eq!(converted_back.kind(), io::ErrorKind::TimedOut);
    }
}
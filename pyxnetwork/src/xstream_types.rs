// src/xstream_types.rs
// Python wrapper types for XStream functionality

use pyo3::prelude::*;
use xstream::types::{XStreamDirection, XStreamState};
use xstream::xstream_error::{ErrorOnRead as RustErrorOnRead, ReadError, XStreamError as RustXStreamError};

/// Python wrapper for stream direction
#[pyclass]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamDirection {
    Inbound = 0,
    Outbound = 1,
}

#[pymethods]
impl StreamDirection {
    pub fn __str__(&self) -> String {
        match self {
            StreamDirection::Inbound => "Inbound".to_string(),
            StreamDirection::Outbound => "Outbound".to_string(),
        }
    }
    
    pub fn __repr__(&self) -> String {
        format!("StreamDirection::{}", self.__str__())
    }
    
    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }
    
    fn __hash__(&self) -> isize {
        *self as isize
    }
}

impl From<XStreamDirection> for StreamDirection {
    fn from(dir: XStreamDirection) -> Self {
        match dir {
            XStreamDirection::Inbound => StreamDirection::Inbound,
            XStreamDirection::Outbound => StreamDirection::Outbound,
        }
    }
}

impl From<StreamDirection> for XStreamDirection {
    fn from(dir: StreamDirection) -> Self {
        match dir {
            StreamDirection::Inbound => XStreamDirection::Inbound,
            StreamDirection::Outbound => XStreamDirection::Outbound,
        }
    }
}

/// Python wrapper for stream state
#[pyclass]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamState {
    Open = 0,
    WriteLocalClosed = 1,
    ReadRemoteClosed = 2,
    LocalClosed = 3,
    RemoteClosed = 4,
    FullyClosed = 5,
    Error = 6,
}

#[pymethods]
impl StreamState {
    fn __str__(&self) -> String {
        match self {
            StreamState::Open => "Open".to_string(),
            StreamState::WriteLocalClosed => "WriteLocalClosed".to_string(),
            StreamState::ReadRemoteClosed => "ReadRemoteClosed".to_string(),
            StreamState::LocalClosed => "LocalClosed".to_string(),
            StreamState::RemoteClosed => "RemoteClosed".to_string(),
            StreamState::FullyClosed => "FullyClosed".to_string(),
            StreamState::Error => "Error".to_string(),
        }
    }
    
    fn __repr__(&self) -> String {
        format!("StreamState::{}", self.__str__())
    }
    
    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }
    
    fn __hash__(&self) -> isize {
        *self as isize
    }
    
    /// Check if the state indicates the stream is closed
    fn is_closed(&self) -> bool {
        matches!(
            self,
            StreamState::LocalClosed
                | StreamState::RemoteClosed
                | StreamState::FullyClosed
                | StreamState::Error
        )
    }
    
    /// Check if the state indicates local closure
    fn is_local_closed(&self) -> bool {
        matches!(
            self,
            StreamState::LocalClosed | StreamState::FullyClosed
        )
    }
    
    /// Check if the state indicates remote closure
    fn is_remote_closed(&self) -> bool {
        matches!(
            self,
            StreamState::RemoteClosed | StreamState::FullyClosed
        )
    }
    
    /// Check if write is locally closed
    fn is_write_local_closed(&self) -> bool {
        matches!(
            self,
            StreamState::WriteLocalClosed
                | StreamState::LocalClosed
                | StreamState::FullyClosed
        )
    }
    
    /// Check if read is remotely closed
    fn is_read_remote_closed(&self) -> bool {
        matches!(
            self,
            StreamState::ReadRemoteClosed
                | StreamState::RemoteClosed
                | StreamState::FullyClosed
        )
    }
}

impl From<XStreamState> for StreamState {
    fn from(state: XStreamState) -> Self {
        match state {
            XStreamState::Open => StreamState::Open,
            XStreamState::WriteLocalClosed => StreamState::WriteLocalClosed,
            XStreamState::ReadRemoteClosed => StreamState::ReadRemoteClosed,
            XStreamState::LocalClosed => StreamState::LocalClosed,
            XStreamState::RemoteClosed => StreamState::RemoteClosed,
            XStreamState::FullyClosed => StreamState::FullyClosed,
            XStreamState::Error => StreamState::Error,
        }
    }
}

/// Python wrapper for XStream error
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyXStreamError {
    /// Raw error data
    data: Vec<u8>,
    /// Human-readable message if data can be interpreted as UTF-8
    message: Option<String>,
}

#[pymethods]
impl PyXStreamError {
    #[new]
    fn new(data: Vec<u8>) -> Self {
        let message = if !data.is_empty() {
            String::from_utf8(data.clone()).ok()
        } else {
            None
        };
        
        Self { data, message }
    }
    
    #[staticmethod]
    fn from_message(message: String) -> Self {
        Self {
            data: message.as_bytes().to_vec(),
            message: Some(message),
        }
    }
    
    #[staticmethod]
    fn from_bytes(data: Vec<u8>) -> Self {
        Self::new(data)
    }
    
    #[getter]
    fn data(&self) -> Vec<u8> {
        self.data.clone()
    }
    
    #[getter]
    fn message(&self) -> Option<String> {
        self.message.clone()
    }
    
    /// Try to interpret error data as UTF-8 string
    fn as_string(&self) -> Option<String> {
        String::from_utf8(self.data.clone()).ok()
    }
    
    /// Check if error is empty
    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Get size of error data
    fn __len__(&self) -> usize {
        self.data.len()
    }
    
    pub fn __str__(&self) -> String {
        if let Some(ref message) = self.message {
            format!("XStreamError: {}", message)
        } else if !self.data.is_empty() {
            format!("XStreamError: {} bytes of binary data", self.data.len())
        } else {
            "XStreamError: empty".to_string()
        }
    }
    
    fn __repr__(&self) -> String {
        if let Some(ref message) = self.message {
            format!("PyXStreamError(message='{}')", message)
        } else {
            format!("PyXStreamError(data={} bytes)", self.data.len())
        }
    }
    
    fn __eq__(&self, other: &Self) -> bool {
        self.data == other.data
    }
    
    fn __hash__(&self) -> isize {
        // Simple hash based on data length and first few bytes
        let mut hash: isize = self.data.len() as isize;
        for (i, &byte) in self.data.iter().take(8).enumerate() {
            hash = hash.wrapping_mul(31).wrapping_add((byte as isize) << (i * 4));
        }
        hash
    }
}

impl From<RustXStreamError> for PyXStreamError {
    fn from(error: RustXStreamError) -> Self {
        Self {
            data: error.data().to_vec(),
            message: error.message().map(|s| s.to_string()),
        }
    }
}

impl From<PyXStreamError> for RustXStreamError {
    fn from(py_error: PyXStreamError) -> Self {
        RustXStreamError::new(py_error.data)
    }
}

/// Python wrapper for ErrorOnRead with partial data
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyErrorOnRead {
    /// Partially read data before error occurred
    partial_data: Vec<u8>,
    /// Type of error: "io" or "xstream"
    error_type: String,
    /// Error message
    error_message: String,
    /// XStream error if this is an XStream error
    xstream_error: Option<PyXStreamError>,
}

#[pymethods]
impl PyErrorOnRead {
    #[new]
    fn new(
        partial_data: Vec<u8>,
        error_type: String,
        error_message: String,
        xstream_error: Option<PyXStreamError>,
    ) -> Self {
        Self {
            partial_data,
            error_type,
            error_message,
            xstream_error,
        }
    }
    
    #[staticmethod]
    pub fn from_io_error(partial_data: Vec<u8>, error_message: String) -> Self {
        Self {
            partial_data,
            error_type: "io".to_string(),
            error_message,
            xstream_error: None,
        }
    }
    
    #[staticmethod]
    fn from_xstream_error(partial_data: Vec<u8>, xstream_error: PyXStreamError) -> Self {
        let error_message = xstream_error.__str__();
        Self {
            partial_data,
            error_type: "xstream".to_string(),
            error_message,
            xstream_error: Some(xstream_error),
        }
    }
    
    #[getter]
    fn partial_data(&self) -> Vec<u8> {
        self.partial_data.clone()
    }
    
    #[getter]
    fn error_type(&self) -> String {
        self.error_type.clone()
    }
    
    #[getter]
    fn error_message(&self) -> String {
        self.error_message.clone()
    }
    
    #[getter]
    fn xstream_error(&self) -> Option<PyXStreamError> {
        self.xstream_error.clone()
    }
    
    /// Check if partial data is available
    fn has_partial_data(&self) -> bool {
        !self.partial_data.is_empty()
    }
    
    /// Get length of partial data
    fn partial_data_len(&self) -> usize {
        self.partial_data.len()
    }
    
    /// Check if this is an IO error
    fn is_io_error(&self) -> bool {
        self.error_type == "io"
    }
    
    /// Check if this is an XStream error
    fn is_xstream_error(&self) -> bool {
        self.error_type == "xstream"
    }
    
    /// Extract partial data (clones the data)
    fn into_partial_data(&self) -> Vec<u8> {
        self.partial_data.clone()
    }
    
    /// Get error details as a dictionary
    fn as_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("partial_data", self.partial_data.clone())?;
        dict.set_item("error_type", &self.error_type)?;
        dict.set_item("error_message", &self.error_message)?;
        dict.set_item("has_partial_data", self.has_partial_data())?;
        dict.set_item("partial_data_len", self.partial_data_len())?;
        
        if let Some(ref xs_error) = self.xstream_error {
            // Convert to PyObject properly
            let py_obj = Py::new(py, xs_error.clone())?;
            dict.set_item("xstream_error", py_obj)?;
        } else {
            dict.set_item("xstream_error", py.None())?;
        }
        
        Ok(dict.to_object(py))
    }
    
    pub fn __str__(&self) -> String {
        if self.has_partial_data() {
            format!(
                "ErrorOnRead: {} error after reading {} bytes: {}",
                self.error_type, self.partial_data_len(), self.error_message
            )
        } else {
            format!(
                "ErrorOnRead: {} error before reading any data: {}",
                self.error_type, self.error_message
            )
        }
    }
    
    fn __repr__(&self) -> String {
        format!(
            "PyErrorOnRead(type='{}', partial_data={} bytes, message='{}')",
            self.error_type,
            self.partial_data_len(),
            if self.error_message.len() > 50 {
                format!("{}...", &self.error_message[..47])
            } else {
                self.error_message.clone()
            }
        )
    }
    
    fn __eq__(&self, other: &Self) -> bool {
        self.partial_data == other.partial_data
            && self.error_type == other.error_type
            && self.error_message == other.error_message
    }
}

impl From<RustErrorOnRead> for PyErrorOnRead {
    fn from(error: RustErrorOnRead) -> Self {
        let (partial_data, error) = error.into_parts();
        
        match error {
            ReadError::Io(io_wrapper) => Self {
                partial_data,
                error_type: "io".to_string(),
                error_message: io_wrapper.message().to_string(),
                xstream_error: None,
            },
            ReadError::XStream(xs_error) => Self {
                partial_data,
                error_type: "xstream".to_string(),
                error_message: xs_error.to_string(),
                xstream_error: Some(PyXStreamError::from(xs_error)),
            },
        }
    }
}

/// Result type for XStream read operations that can contain errors with partial data
pub type XStreamReadResult<T> = Result<T, PyErrorOnRead>;

/// Utility functions for working with XStream errors
#[pyclass]
pub struct XStreamErrorUtils;

#[pymethods]
impl XStreamErrorUtils {
    #[staticmethod]
    fn is_critical_io_error(error_message: &str) -> bool {
        let lower_msg = error_message.to_lowercase();
        lower_msg.contains("connection reset")
            || lower_msg.contains("connection aborted")
            || lower_msg.contains("broken pipe")
            || lower_msg.contains("not connected")
    }
    
    #[staticmethod]
    fn categorize_error(error: &PyErrorOnRead) -> String {
        if error.is_xstream_error() {
            "xstream".to_string()
        } else if Self::is_critical_io_error(&error.error_message) {
            "critical_io".to_string()
        } else {
            "io".to_string()
        }
    }
    
    #[staticmethod]
    pub fn create_timeout_error(partial_data: Vec<u8>) -> PyErrorOnRead {
        PyErrorOnRead::from_io_error(
            partial_data,
            "Operation timed out".to_string(),
        )
    }
    
    #[staticmethod]
    fn create_eof_error(partial_data: Vec<u8>) -> PyErrorOnRead {
        PyErrorOnRead::from_io_error(
            partial_data,
            "Unexpected end of file".to_string(),
        )
    }
    
    #[staticmethod]
    fn create_permission_error(operation: &str) -> PyErrorOnRead {
        PyErrorOnRead::from_io_error(
            Vec::new(),
            format!("Permission denied: {}", operation),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_direction_conversion() {
        let rust_inbound = XStreamDirection::Inbound;
        let py_inbound = StreamDirection::from(rust_inbound);
        assert_eq!(py_inbound, StreamDirection::Inbound);
        
        let rust_outbound = XStreamDirection::Outbound;
        let py_outbound = StreamDirection::from(rust_outbound);
        assert_eq!(py_outbound, StreamDirection::Outbound);
    }
    
    #[test]
    fn test_stream_state_methods() {
        assert!(!StreamState::Open.is_closed());
        assert!(!StreamState::WriteLocalClosed.is_closed());
        assert!(StreamState::FullyClosed.is_closed());
        assert!(StreamState::Error.is_closed());
        
        assert!(StreamState::WriteLocalClosed.is_write_local_closed());
        assert!(StreamState::ReadRemoteClosed.is_read_remote_closed());
    }
    
    #[test]
    fn test_xstream_error_creation() {
        let error = PyXStreamError::new(b"test error".to_vec());
        assert_eq!(error.data(), b"test error".to_vec());
        assert_eq!(error.message(), Some("test error".to_string()));
        assert!(!error.is_empty());
        assert_eq!(error.__len__(), 10);
        
        let empty_error = PyXStreamError::new(Vec::new());
        assert!(empty_error.is_empty());
        assert_eq!(empty_error.__len__(), 0);
    }
    
    #[test]
    fn test_error_on_read_creation() {
        let partial_data = b"partial".to_vec();
        let error = PyErrorOnRead::from_io_error(
            partial_data.clone(),
            "Test error".to_string(),
        );
        
        assert_eq!(error.partial_data(), partial_data);
        assert!(error.has_partial_data());
        assert_eq!(error.partial_data_len(), 7);
        assert!(error.is_io_error());
        assert!(!error.is_xstream_error());
    }
}
// src/xstream.rs
// XStream implementation for Python wrapper using separate types

use libp2p::PeerId;
use pyo3::exceptions::{PyIOError, PyTimeoutError, PyPermissionError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3_asyncio::tokio::future_into_py;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;

use crate::types::PeerId as PyPeerId;
use crate::xstream_types::{
    StreamDirection, StreamState, PyXStreamError, PyErrorOnRead, 
    XStreamReadResult, XStreamErrorUtils
};

use xstream::events::XStreamEvent;
use xstream::types::{XStreamDirection, XStreamID};
use xstream::xstream::XStream as RustXStream;

#[pyclass]
pub struct XStream {
    inner: Arc<TokioMutex<Option<RustXStream>>>,
    runtime: Arc<Runtime>,
    peer_id: PyPeerId,
    stream_id: u128,
    direction: StreamDirection,
}

impl XStream {
    /// Create XStream from Rust XStream
    pub fn from_xstream(stream: RustXStream) -> Self {
        let peer_id = PyPeerId {
            inner: stream.peer_id.clone(),
        };
        let stream_id: u128 = stream.id.into();
        let direction: StreamDirection = stream.direction.into();

        Self {
            inner: Arc::new(TokioMutex::new(Some(stream))),
            runtime: Arc::new(Runtime::new().expect("Failed to create tokio runtime")),
            peer_id,
            stream_id,
            direction,
        }
    }

    /// Create XStream from Arc<RustXStream>
    pub fn from_arc_xstream(stream: Arc<RustXStream>) -> Self {
        let peer_id = PyPeerId {
            inner: stream.peer_id.clone(),
        };
        let stream_id: u128 = stream.id.into();
        let direction: StreamDirection = stream.direction.into();
        let stream_clone = stream.clone();

        Self {
            inner: Arc::new(TokioMutex::new(Some((*stream_clone).clone()))),
            runtime: Arc::new(Runtime::new().expect("Failed to create tokio runtime")),
            peer_id,
            stream_id,
            direction,
        }
    }
}

#[pymethods]
impl XStream {
    #[getter]
    fn peer_id(&self) -> PyPeerId {
        self.peer_id.clone()
    }

    #[getter]
    fn id(&self) -> u128 {
        self.stream_id
    }

    #[getter]
    fn direction(&self) -> StreamDirection {
        self.direction
    }
    
    // State management methods
    fn state<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        
        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(StreamState::from(stream.state())),
                None => Ok(StreamState::FullyClosed),
            }
        })
    }
    
    fn is_closed<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.is_closed()),
                None => Ok(true),
            }
        })
    }
    
    fn is_local_closed<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.is_local_closed()),
                None => Ok(true),
            }
        })
    }
    
    fn is_remote_closed<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.is_remote_closed()),
                None => Ok(true),
            }
        })
    }
    
    fn is_write_local_closed<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.is_write_local_closed()),
                None => Ok(true),
            }
        })
    }
    
    fn is_read_remote_closed<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.is_read_remote_closed()),
                None => Ok(true),
            }
        })
    }

    // Enhanced read methods with error handling
    fn read<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => {
                    let error = PyErrorOnRead::from_io_error(
                        Vec::new(),
                        "Stream is closed".to_string(),
                    );
                    return Err(PyErr::new::<PyIOError, _>(error.__str__()));
                }
            };

            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read()).await {
                    Ok(res) => res,
                    Err(_) => {
                        let timeout_error = XStreamErrorUtils::create_timeout_error(Vec::new());
                        return Err(PyErr::new::<PyTimeoutError, _>(timeout_error.__str__()));
                    }
                }
            } else {
                stream.read().await
            };

            Python::with_gil(|py| match result {
                Ok(data) => {
                    let py_bytes = PyBytes::new(py, &data);
                    Ok(py_bytes.to_object(py))
                }
                Err(error_on_read) => {
                    let py_error = PyErrorOnRead::from(error_on_read);
                    Err(PyErr::new::<PyIOError, _>(py_error.__str__()))
                }
            })
        })
    }

    fn read_exact<'py>(
        &self,
        py: Python<'py>,
        size: usize,
        timeout_ms: Option<u64>,
    ) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => {
                    let error = PyErrorOnRead::from_io_error(
                        Vec::new(),
                        "Stream is closed".to_string(),
                    );
                    return Err(PyErr::new::<PyIOError, _>(error.__str__()));
                }
            };

            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read_exact(size)).await {
                    Ok(res) => res,
                    Err(_) => {
                        let timeout_error = XStreamErrorUtils::create_timeout_error(Vec::new());
                        return Err(PyErr::new::<PyTimeoutError, _>(timeout_error.__str__()));
                    }
                }
            } else {
                stream.read_exact(size).await
            };

            Python::with_gil(|py| match result {
                Ok(data) => {
                    let py_bytes = PyBytes::new(py, &data);
                    Ok(py_bytes.to_object(py))
                }
                Err(error_on_read) => {
                    let py_error = PyErrorOnRead::from(error_on_read);
                    Err(PyErr::new::<PyIOError, _>(py_error.__str__()))
                }
            })
        })
    }

    fn read_to_end<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => {
                    let error = PyErrorOnRead::from_io_error(
                        Vec::new(),
                        "Stream is closed".to_string(),
                    );
                    return Err(PyErr::new::<PyIOError, _>(error.__str__()));
                }
            };

            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read_to_end()).await {
                    Ok(res) => res,
                    Err(_) => {
                        let timeout_error = XStreamErrorUtils::create_timeout_error(Vec::new());
                        return Err(PyErr::new::<PyTimeoutError, _>(timeout_error.__str__()));
                    }
                }
            } else {
                stream.read_to_end().await
            };

            Python::with_gil(|py| match result {
                Ok(data) => {
                    let py_bytes = PyBytes::new(py, &data);
                    Ok(py_bytes.to_object(py))
                }
                Err(error_on_read) => {
                    let py_error = PyErrorOnRead::from(error_on_read);
                    Err(PyErr::new::<PyIOError, _>(py_error.__str__()))
                }
            })
        })
    }
    
    // Compatibility methods that ignore XStream errors
    fn read_ignore_errors<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read_ignore_errors()).await {
                    Ok(res) => res,
                    Err(_) => return Err(PyErr::new::<PyTimeoutError, _>("Read operation timed out")),
                }
            } else {
                stream.read_ignore_errors().await
            };

            Python::with_gil(|py| match result {
                Ok(data) => {
                    let py_bytes = PyBytes::new(py, &data);
                    Ok(py_bytes.to_object(py))
                }
                Err(e) => Err(PyErr::new::<PyIOError, _>(format!(
                    "Failed to read from stream: {}",
                    e
                ))),
            })
        })
    }
    
    fn read_to_end_ignore_errors<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read_to_end_ignore_errors()).await {
                    Ok(res) => res,
                    Err(_) => return Err(PyErr::new::<PyTimeoutError, _>("Read operation timed out")),
                }
            } else {
                stream.read_to_end_ignore_errors().await
            };

            Python::with_gil(|py| match result {
                Ok(data) => {
                    let py_bytes = PyBytes::new(py, &data);
                    Ok(py_bytes.to_object(py))
                }
                Err(e) => Err(PyErr::new::<PyIOError, _>(format!(
                    "Failed to read to end of stream: {}",
                    e
                ))),
            })
        })
    }

    // Write methods
    fn write<'py>(&self, py: Python<'py>, data: &PyAny) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        // Convert PyAny to Vec<u8>
        let bytes = if let Ok(bytes) = data.extract::<Vec<u8>>() {
            bytes
        } else if let Ok(string) = data.extract::<String>() {
            string.into_bytes()
        } else {
            return Err(PyErr::new::<PyValueError, _>("Data must be bytes or string"));
        };

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            match stream.write_all(bytes).await {
                Ok(_) => Ok(()),
                Err(e) => Err(PyErr::new::<PyIOError, _>(format!(
                    "Failed to write to stream: {}",
                    e
                ))),
            }
        })
    }
    
    fn write_all<'py>(&self, py: Python<'py>, data: &PyAny) -> PyResult<&'py PyAny> {
        // Alias for write for compatibility
        self.write(py, data)
    }
    
    fn flush<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            match stream.flush().await {
                Ok(_) => Ok(()),
                Err(e) => Err(PyErr::new::<PyIOError, _>(format!(
                    "Failed to flush stream: {}",
                    e
                ))),
            }
        })
    }

    fn write_eof<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            match stream.write_eof().await {
                Ok(_) => Ok(()),
                Err(e) => Err(PyErr::new::<PyIOError, _>(format!(
                    "Failed to send EOF: {}",
                    e
                ))),
            }
        })
    }

    // Error stream methods
    fn error_read<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.error_read()).await {
                    Ok(res) => res,
                    Err(_) => return Err(PyErr::new::<PyTimeoutError, _>("Read operation timed out")),
                }
            } else {
                stream.error_read().await
            };

            Python::with_gil(|py| match result {
                Ok(data) => {
                    let py_bytes = PyBytes::new(py, &data);
                    Ok(py_bytes.to_object(py))
                }
                Err(e) => Err(PyErr::new::<PyIOError, _>(format!(
                    "Failed to read from error stream: {}",
                    e
                ))),
            })
        })
    }

    fn error_write<'py>(&self, py: Python<'py>, data: &PyAny) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        // Convert PyAny to Vec<u8>
        let bytes = if let Ok(bytes) = data.extract::<Vec<u8>>() {
            bytes
        } else if let Ok(string) = data.extract::<String>() {
            string.into_bytes()
        } else {
            return Err(PyErr::new::<PyValueError, _>("Data must be bytes or string"));
        };

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };

            match stream.error_write(bytes).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    // Handle specific error types with appropriate Python errors
                    match e.kind() {
                        std::io::ErrorKind::PermissionDenied => {
                            Err(PyErr::new::<PyPermissionError, _>(
                                "Only inbound streams can write to error stream"
                            ))
                        }
                        std::io::ErrorKind::AlreadyExists => Err(PyErr::new::<PyIOError, _>(
                            "Error already written to this stream",
                        )),
                        _ => Err(PyErr::new::<PyIOError, _>(format!(
                            "Failed to write to error stream: {}",
                            e
                        ))),
                    }
                }
            }
        })
    }
    
    // Error data availability methods
    fn has_error_data<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.has_error_data().await),
                None => Ok(false),
            }
        })
    }
    
    fn has_pending_error<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => Ok(stream.has_pending_error().await),
                None => Ok(false),
            }
        })
    }
    
    fn get_cached_error<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let inner_guard = inner.lock().await;
            match &*inner_guard {
                Some(stream) => {
                    if let Some(error_data) = stream.get_cached_error().await {
                        Python::with_gil(|py| {
                            let py_bytes = PyBytes::new(py, &error_data);
                            Ok(Some(py_bytes.to_object(py)))
                        })
                    } else {
                        Ok(None)
                    }
                }
                None => Ok(None),
            }
        })
    }

    fn close<'py>(&mut self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let mut inner_guard = inner.lock().await;
            let stream_opt = inner_guard.take();

            if let Some(mut stream) = stream_opt {
                match stream.close().await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        // Put the stream back since close failed
                        *inner_guard = Some(stream);
                        Err(PyErr::new::<PyIOError, _>(format!(
                            "Failed to close stream: {}",
                            e
                        )))
                    }
                }
            } else {
                // Already closed
                Ok(())
            }
        })
    }

    fn __str__(&self) -> String {
        format!(
            "XStream(id={}, peer={}, direction={})",
            self.stream_id,
            self.peer_id,
            self.direction.__str__()
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "XStream(id={}, peer_id='{}', direction={})",
            self.stream_id,
            self.peer_id,
            self.direction.__repr__()
        )
    }

    fn __enter__(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }

    fn __exit__(
        &mut self,
        py: Python,
        _exc_type: &PyAny,
        _exc_value: &PyAny,
        _traceback: &PyAny,
    ) -> PyResult<bool> {
        let _fut = self.close(py)?;
        // We need to wait for the future to complete
        py.allow_threads(|| {
            self.runtime.block_on(async {
                // Wait a bit for the Python future to resolve
                tokio::time::sleep(Duration::from_millis(100)).await;
            });
        });
        Ok(false) // Don't suppress exceptions
    }
}
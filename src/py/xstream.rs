// src/py/xstream.rs
use pyo3::prelude::*;
use pyo3::exceptions::PyIOError;
use pyo3::types::PyBytes;
use std::sync::{Arc};
use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;
use std::time::Duration;
use pyo3_asyncio::tokio::future_into_py;

use crate::network::xstream::xstream::XStream as RustXStream;
use crate::py::types::PeerId;

#[pyclass]
pub struct XStream {
    inner: Arc<TokioMutex<Option<RustXStream>>>,
    runtime: Arc<Runtime>,
    peer_id: PeerId,
    stream_id: u128,
}

impl XStream {
    // Create XStream from Rust XStream
    pub fn from_xstream(stream: RustXStream) -> Self {
        let peer_id = PeerId { inner: stream.peer_id.clone() };
        let stream_id = stream.id;
        
        Self {
            inner: Arc::new(TokioMutex::new(Some(stream))),
            runtime: Arc::new(Runtime::new().expect("Failed to create tokio runtime")),
            peer_id,
            stream_id,
        }
    }
    
    // Create XStream from Arc<RustXStream>
    pub fn from_arc_xstream(stream: Arc<RustXStream>) -> Self {
        let peer_id = PeerId { inner: stream.peer_id.clone() };
        let stream_id = stream.id;
        let stream_clone = stream.clone();
        
        Self {
            inner: Arc::new(TokioMutex::new(Some((*stream_clone).clone()))),
            runtime: Arc::new(Runtime::new().expect("Failed to create tokio runtime")),
            peer_id,
            stream_id,
        }
    }
}

#[pymethods]
impl XStream {
    #[getter]
    fn peer_id(&self) -> PeerId {
        self.peer_id.clone()
    }
    
    #[getter]
    fn id(&self) -> u128 {
        self.stream_id
    }
    
    fn write<'py>(&self, py: Python<'py>, data: &PyAny) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        
        // Convert PyAny to Vec<u8>
        let bytes = if let Ok(bytes) = data.extract::<Vec<u8>>() {
            bytes
        } else if let Ok(string) = data.extract::<String>() {
            string.into_bytes()
        } else {
            return Err(PyErr::new::<PyIOError, _>("Data must be bytes or string"));
        };
        
        future_into_py(py, async move {
            // Get the stream
            let mut inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };
            
            // Write data to the stream
            let result = stream.write_all(bytes).await;
            
            // Return the result
            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(PyErr::new::<PyIOError, _>(
                    format!("Failed to write to stream: {}", e)
                )),
            }
        })
    }
    
    fn read<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));
        
        future_into_py(py, async move {
            // Get the stream
            let mut inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };
            
            // Read data with optional timeout
            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read()).await {
                    Ok(res) => res,
                    Err(_) => return Err(PyErr::new::<PyIOError, _>("Read operation timed out")),
                }
            } else {
                stream.read().await
            };
            
            // Convert Vec<u8> to PyBytes and return
            Python::with_gil(|py| {
                match result {
                    Ok(data) => {
                        let py_bytes = PyBytes::new(py, &data);
                        Ok(py_bytes.to_object(py))
                    },
                    Err(e) => Err(PyErr::new::<PyIOError, _>(
                        format!("Failed to read from stream: {}", e)
                    )),
                }
            })
        })
    }
    
    fn read_exact<'py>(&self, py: Python<'py>, size: usize, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));
        
        future_into_py(py, async move {
            // Get the stream
            let mut inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };
            
            // Read exact amount of data with optional timeout
            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read_exact(size)).await {
                    Ok(res) => res,
                    Err(_) => return Err(PyErr::new::<PyIOError, _>("Read operation timed out")),
                }
            } else {
                stream.read_exact(size).await
            };
            
            // Convert Vec<u8> to PyBytes and return
            Python::with_gil(|py| {
                match result {
                    Ok(data) => {
                        let py_bytes = PyBytes::new(py, &data);
                        Ok(py_bytes.to_object(py))
                    },
                    Err(e) => Err(PyErr::new::<PyIOError, _>(
                        format!("Failed to read exact bytes from stream: {}", e)
                    )),
                }
            })
        })
    }
    
    fn read_to_end<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));
        
        future_into_py(py, async move {
            // Get the stream
            let mut inner_guard = inner.lock().await;
            let stream = match &*inner_guard {
                Some(s) => s,
                None => return Err(PyErr::new::<PyIOError, _>("Stream is closed")),
            };
            
            // Read all data with optional timeout
            let result = if let Some(duration) = timeout {
                match tokio::time::timeout(duration, stream.read_to_end()).await {
                    Ok(res) => res,
                    Err(_) => return Err(PyErr::new::<PyIOError, _>("Read operation timed out")),
                }
            } else {
                stream.read_to_end().await
            };
            
            // Convert Vec<u8> to PyBytes and return
            Python::with_gil(|py| {
                match result {
                    Ok(data) => {
                        let py_bytes = PyBytes::new(py, &data);
                        Ok(py_bytes.to_object(py))
                    },
                    Err(e) => Err(PyErr::new::<PyIOError, _>(
                        format!("Failed to read to end of stream: {}", e)
                    )),
                }
            })
        })
    }
    
    fn close<'py>(&mut self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        
        future_into_py(py, async move {
            // Take the stream out (replacing with None)
            let mut inner_guard = inner.lock().await;
            let stream_opt = inner_guard.take();
            
            // Close the stream if it exists
            if let Some(mut stream) = stream_opt {
                match stream.close().await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        // Put the stream back since close failed
                        *inner_guard = Some(stream);
                        Err(PyErr::new::<PyIOError, _>(
                            format!("Failed to close stream: {}", e)
                        ))
                    }
                }
            } else {
                // Already closed
                Ok(())
            }
        })
    }
    
    fn is_closed<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let inner = self.inner.clone();
        
        future_into_py(py, async move {
            // Check if the inner stream is None or marked as closed
            let inner_guard = inner.lock().await;
            
            match &*inner_guard {
                Some(stream) => Ok(stream.is_closed()),
                None => Ok(true), // If inner is None, it's closed
            }
        })
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
                // This is hacky, in real code you would await the future properly
                tokio::time::sleep(Duration::from_millis(100)).await;
            });
        });
        Ok(false) // Don't suppress exceptions
    }
}
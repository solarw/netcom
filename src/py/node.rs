// src/py/node.rs
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use pyo3::prelude::*;
use pyo3::exceptions::{PyRuntimeError, PyValueError, PyTimeoutError};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, Mutex};
use libp2p::Multiaddr;
use tracing::{debug, info, error, warn};
use pyo3_asyncio::tokio::future_into_py;

use crate::network::{
    commander::Commander,
    commands::NetworkCommand,
    events::NetworkEvent,
    node::NetworkNode,
    utils::make_new_key,
    xauth::por::por::{PorUtils, ProofOfRepresentation},
};

use crate::py::{
    types::{KeyPair, PeerId as PyPeerId},
    xstream::XStream as PyXStream,
    events::network_event_to_dict,
};

/// Python class representing a P2P Network Node
#[pyclass]
pub struct Node {
    /// The runtime for executing async tasks
    runtime: Arc<Runtime>,
    /// The commander for sending commands to the node
    commander: Arc<Mutex<Option<Commander>>>,
    /// Channel for sending commands to the node
    cmd_tx: Arc<Mutex<Option<mpsc::Sender<NetworkCommand>>>>,
    /// Queue of received events
    event_queue: Arc<Mutex<VecDeque<NetworkEvent>>>,
    /// Local peer ID
    peer_id: PyPeerId,
    /// Is the node running?
    running: Arc<Mutex<bool>>,
    /// Keypair for the node
    key_pair: KeyPair,
}

#[pymethods]
impl Node {
    #[new]
    fn new(key_pair: KeyPair, _enable_mdns: bool, _kad_server_mode: bool) -> Self {
        // Create a new tokio runtime
        let runtime = Runtime::new().expect("Failed to create tokio runtime");
        
        // Create a new instance of Node with empty fields
        Self {
            runtime: Arc::new(runtime),
            commander: Arc::new(Mutex::new(None)),
            cmd_tx: Arc::new(Mutex::new(None)),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            peer_id: key_pair.to_peer_id(),
            running: Arc::new(Mutex::new(false)),
            key_pair,
        }
    }
    
    /// Start the node
    fn start<'py>(&mut self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let _runtime = self.runtime.clone();
        let event_queue = self.event_queue.clone();
        let running = self.running.clone();
        let cmd_tx_mutex = self.cmd_tx.clone();
        let commander_mutex = self.commander.clone();
        
        // Clone the key_pair
        let key_pair = self.key_pair.inner.clone();
        
        future_into_py(py, async move {
            // Check if already running
            let is_running = *running.lock().await;
            if is_running {
                return Err(PyErr::new::<PyRuntimeError, _>("Node already started"));
            }
            
            // Create proof of representation with 24 hour expiry
            let owner_keypair = PorUtils::generate_owner_keypair();
            let por = ProofOfRepresentation::create(
                &owner_keypair,
                key_pair.public().to_peer_id(),
                Duration::from_secs(86400),
            ).expect("Failed to create Proof of Representation");
            
            // Create the network node
            let enable_mdns = false; // We'll control this with explicit commands
            let kad_server_mode = false; // We'll control this with explicit commands
            
            let result = NetworkNode::new(
                key_pair,
                por,
                enable_mdns,
                kad_server_mode,
            ).await;
            
            match result {
                Ok((mut node, cmd_tx, mut event_rx, peer_id)) => {
                    info!("Node created with peer ID: {}", peer_id);
                    
                    // Store the command sender
                    *cmd_tx_mutex.lock().await = Some(cmd_tx.clone());
                    
                    // Create a commander
                    let cmd = Commander::new(cmd_tx);
                    *commander_mutex.lock().await = Some(cmd);
                    
                    // Set running flag
                    *running.lock().await = true;
                    
                    // Start event handling task
                    let event_queue_clone = event_queue.clone();
                    let running_clone = running.clone();
                    
                    // Spawn a task to receive events from the node
                    tokio::spawn(async move {
                        while let Some(event) = event_rx.recv().await {
                            // Print the event for debugging
                            debug!("Received event: {:?}", event);
                            
                            // Add the event to the queue
                            event_queue_clone.lock().await.push_back(event);
                        }
                        
                        // If we're here, event channel has closed, set running to false
                        *running_clone.lock().await = false;
                        info!("Event channel closed, node is no longer running");
                    });
                    
                    // Spawn the node's run task
                    tokio::spawn(async move {
                        node.run().await;
                        info!("Node run loop exited");
                    });
                    
                    Ok(peer_id.to_string())
                },
                Err(e) => {
                    error!("Failed to create node: {}", e);
                    Err(PyErr::new::<PyRuntimeError, _>(format!("Failed to create node: {}", e)))
                }
            }
        })
    }
    
    /// Stop the node
    fn stop<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let cmd_tx = self.cmd_tx.clone();
        let running = self.running.clone();
        
        future_into_py(py, async move {
            // Check if running
            let is_running = *running.lock().await;
            if !is_running {
                return Ok(());
            }
            
            // Get the command sender
            let mut cmd_tx_guard = cmd_tx.lock().await;
            if let Some(tx) = &*cmd_tx_guard {
                // Send shutdown command
                if let Err(e) = tx.send(NetworkCommand::Shutdown).await {
                    error!("Failed to send shutdown command: {}", e);
                    return Err(PyErr::new::<PyRuntimeError, _>(
                        format!("Failed to send shutdown command: {}", e)
                    ));
                }
                
                // Clear the command sender
                *cmd_tx_guard = None;
            }
            
            // Set running flag to false
            *running.lock().await = false;
            
            Ok(())
        })
    }
    
    /// Get the next event from the node, with optional timeout
    fn get_next_event<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<&'py PyAny> {
        let event_queue = self.event_queue.clone();
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms));
        
        future_into_py(py, async move {
            // Try to get an event from the queue
            let mut event_queue_guard = event_queue.lock().await;
            
            // If there's an event in the queue, return it immediately
            if let Some(event) = event_queue_guard.pop_front() {
                return Ok(Some(network_event_to_dict(event)?));
            }
            
            // If no event and no timeout, return None
            if timeout.is_none() {
                return Ok(None);
            }
            
            // Drop the guard to allow other tasks to add events
            drop(event_queue_guard);
            
            // Wait for timeout duration with periodic checks
            let start = std::time::Instant::now();
            while let Some(timeout_duration) = timeout {
                if start.elapsed() >= timeout_duration {
                    break;
                }
                
                // Sleep for a short duration
                tokio::time::sleep(Duration::from_millis(50)).await;
                
                // Check if any events have been added
                let mut event_queue_guard = event_queue.lock().await;
                if let Some(event) = event_queue_guard.pop_front() {
                    return Ok(Some(network_event_to_dict(event)?));
                }
                
                // Drop the guard to allow other tasks to add events
                drop(event_queue_guard);
            }
            
            // No events within timeout
            Ok(None)
        })
    }
    
    /// Listen on a specific port or random port if port=0
    fn listen_port<'py>(&self, py: Python<'py>, port: u16, host: String) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Call listen_port on the commander
            match cmd.listen_port(Some(host.clone()), port).await {
                Ok(()) => Ok(format!("/ip4/{}/udp/{}/quic-v1", host, port)),
                Err(e) => Err(PyErr::new::<PyRuntimeError, _>(
                    format!("Failed to listen on port {}: {}", port, e)
                )),
            }
        })
    }
    
    /// Connect to a peer using multiaddr
    fn connect<'py>(&self, py: Python<'py>, addr: &str) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        let addr_string = addr.to_string();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Parse multiaddr
            let multiaddr = match addr_string.parse::<Multiaddr>() {
                Ok(addr) => addr,
                Err(e) => return Err(PyErr::new::<PyValueError, _>(
                    format!("Invalid multiaddress: {}", e)
                )),
            };
            
            // Call connect on the commander
            match cmd.connect(multiaddr).await {
                Ok(()) => Ok(true),
                Err(e) => {
                    error!("Failed to connect: {}", e);
                    Ok(false)
                },
            }
        })
    }
    
    /// Disconnect from a peer
    fn disconnect<'py>(&self, py: Python<'py>, peer_id: &PyPeerId) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        let peer_id = peer_id.inner.clone();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Call disconnect on the commander
            match cmd.disconnect(peer_id).await {
                Ok(()) => Ok(true),
                Err(e) => {
                    error!("Failed to disconnect: {}", e);
                    Ok(false)
                },
            }
        })
    }
    
    /// Find a peer using Kademlia
    fn find<'py>(&self, py: Python<'py>, peer_id: &PyPeerId) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        let peer_id = peer_id.inner.clone();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Enable Kademlia and bootstrap before searching
            if let Err(e) = cmd.bootstrap_kad().await {
                warn!("Failed to bootstrap Kademlia: {}", e);
            }
            
            // Call find_peer_addresses on the commander
            match cmd.find_peer_addresses(peer_id).await {
                Ok(()) => Ok(true),
                Err(e) => {
                    error!("Failed to find peer: {}", e);
                    Ok(false)
                },
            }
        })
    }
    
    /// Bootstrap Kademlia DHT
    fn bootstrap_kad<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Call bootstrap_kad on the commander
            match cmd.bootstrap_kad().await {
                Ok(()) => Ok(true),
                Err(e) => {
                    error!("Failed to bootstrap Kademlia: {}", e);
                    Ok(false)
                },
            }
        })
    }
    
    /// Search for a peer's addresses using Kademlia
    fn search_peer_addresses<'py>(&self, py: Python<'py>, peer_id: &PyPeerId) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        let peer_id = peer_id.inner.clone();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Call search_peer_addresses on the commander
            match cmd.search_peer_addresses(peer_id).await {
                Ok(addresses) => {
                    let addr_strings: Vec<String> = addresses.iter()
                        .map(|addr| addr.to_string())
                        .collect();
                    Ok(addr_strings)
                },
                Err(e) => {
                    error!("Failed to search for peer addresses: {}", e);
                    Ok(Vec::<String>::new())
                },
            }
        })
    }
    
    /// Open a stream to a peer
    fn open_stream<'py>(&self, py: Python<'py>, peer_id: &PyPeerId) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        let peer_id = peer_id.inner.clone();
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Call open_stream on the commander
            match cmd.open_stream(peer_id).await {
                Ok(stream) => {
                    let py_stream = PyXStream::from_xstream(stream);
                    Ok(py_stream)
                },
                Err(e) => {
                    error!("Failed to open stream: {}", e);
                    Err(PyErr::new::<PyRuntimeError, _>(
                        format!("Failed to open stream: {}", e)
                    ))
                },
            }
        })
    }
    
    /// Send a message to a peer via stream
    fn stream_message<'py>(&self, py: Python<'py>, peer_id: &PyPeerId, message: &str, timeout: u64) -> PyResult<&'py PyAny> {
        let commander = self.commander.clone();
        let peer_id = peer_id.inner.clone();
        let message = message.to_string();
        let timeout_duration = Duration::from_secs(timeout);
        
        future_into_py(py, async move {
            let commander_guard = commander.lock().await;
            
            // Check if the commander exists
            let cmd = match &*commander_guard {
                Some(cmd) => cmd,
                None => return Err(PyErr::new::<PyRuntimeError, _>("Node not started")),
            };
            
            // Set up timeout
            let stream_task = async {
                // Open a stream
                let result = cmd.open_stream(peer_id).await;
                
                match result {
                    Ok(mut stream) => {
                        // Send the message
                        let message_bytes = message.into_bytes();
                        let write_result = stream.write_all(message_bytes).await;
                        
                        // Close the stream
                        let _ = stream.close().await;
                        
                        write_result.map_err(|e| e.to_string())
                    },
                    Err(e) => Err(e),
                }
            };
            
            // Wait for the task with timeout
            match tokio::time::timeout(timeout_duration, stream_task).await {
                Ok(Ok(_)) => Ok(true),
                Ok(Err(e)) => {
                    error!("Failed to send message: {}", e);
                    Ok(false)
                },
                Err(_) => {
                    error!("Timeout while sending message");
                    Err(PyErr::new::<PyTimeoutError, _>("Timeout while sending message"))
                },
            }
        })
    }
    
    /// Get the local peer ID
    fn peer_id(&self) -> PyResult<PyPeerId> {
        Ok(self.peer_id.clone())
    }
    
    /// Get info about the node
    fn info(&self) -> PyResult<String> {
        let peer_id = self.peer_id.to_string();
        
        let running = self.runtime.block_on(async {
            *self.running.lock().await
        });
        
        Ok(format!(
            "Node: Peer ID: {}, Running: {}", 
            peer_id, 
            running
        ))
    }
    
    // Python context manager methods
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
        // Stop the node when exiting context
        let _fut = self.stop(py)?;
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
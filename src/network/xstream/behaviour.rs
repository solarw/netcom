use super::consts::XSTREAM_PROTOCOL;
use libp2p::{
    core::transport::PortUse,
    swarm::{derive_prelude::*, ConnectionId, NetworkBehaviour, NotifyHandler, ToSwarm},
    Multiaddr, PeerId, StreamProtocol,
};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use std::task::{Context, Poll};
use tracing::{debug, warn, info, trace, error};

use super::events::XStreamEvent;
use super::handler::{XStreamHandler, XStreamHandlerEvent, XStreamHandlerIn};
use super::xstream::XStream;

/// NetworkBehaviour for working with XStream
pub struct XStreamNetworkBehaviour {
    /// Mapping (peer_id, stream_id) -> XStream
    streams: HashMap<(PeerId, u128), XStream>,
    /// Events waiting to be processed
    events: Vec<ToSwarm<XStreamEvent, XStreamHandlerIn>>,
    /// Pending stream openings
    pending_streams: HashMap<PeerId, oneshot::Sender<Result<XStream, String>>>,
    /// Channel for stream closure notifications - sender only
    closure_sender: mpsc::UnboundedSender<(PeerId, u128)>,
    /// Receiver for events from the dedicated closure task
    stream_close_events: mpsc::UnboundedReceiver<XStreamEvent>,
}

impl XStreamNetworkBehaviour {
    /// Creates a new XStreamNetworkBehaviour
    pub fn new() -> Self {
        // Channel for closure notifications
        let (closure_sender, mut closure_receiver) = mpsc::unbounded_channel();
        
        // Channel for events from dedicated task to behavior
        let (event_sender, stream_close_events) = mpsc::unbounded_channel();
        
        tokio::spawn(async move {
            info!("[CLOSURE_TASK] Started dedicated stream closure monitoring task");
            
            while let Some((peer_id, stream_id)) = closure_receiver.recv().await {
                info!("[CLOSURE_TASK] Received closure notification for stream {} from peer {}", stream_id, peer_id);
                
                // Send an event to the behavior
                match event_sender.send(XStreamEvent::StreamClosed {
                    peer_id,
                    stream_id,
                }) {
                    Ok(_) => info!("[CLOSURE_TASK] Successfully sent StreamClosed event to behavior for stream {}", stream_id),
                    Err(e) => error!("[CLOSURE_TASK] Failed to send StreamClosed event: {}", e),
                }
            }
            
            warn!("[CLOSURE_TASK] Stream closure monitoring task exited - channel closed");
        });
        
        Self {
            streams: HashMap::new(),
            events: Vec::new(),
            pending_streams: HashMap::new(),
            closure_sender,
            stream_close_events,
        }
    }


    /// Requests to open a new stream to the specified peer
    pub fn request_open_stream(&mut self, peer_id: PeerId) {
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: XStreamHandlerIn::OpenStream,
        });
    }

    /// Asynchronously opens a new stream and returns XStream or an error
    pub async fn open_stream(
        &mut self,
        peer_id: PeerId,
        response: oneshot::Sender<Result<XStream, String>>,
    ) {
        // Add to pending streams map
        self.pending_streams.insert(peer_id, response);

        // Request stream opening
        self.request_open_stream(peer_id);
    }

    /// Gets an XStream by peer ID and stream ID
    pub fn get_stream(&self, peer_id: PeerId, stream_id: u128) -> Option<&XStream> {
        self.streams.get(&(peer_id, stream_id))
    }

    /// Gets a mutable XStream by peer ID and stream ID
    pub fn get_stream_mut(&mut self, peer_id: PeerId, stream_id: u128) -> Option<&mut XStream> {
        self.streams.get_mut(&(peer_id, stream_id))
    }

    /// Notifies that a stream is closed
    pub fn notify_stream_closed(&mut self, peer_id: PeerId, stream_id: u128) {
        debug!("Manual notification of stream closure: {}", stream_id);
        // Remove the stream from the active streams map
        self.streams.remove(&(peer_id, stream_id));
        
        // Generate the appropriate event
        self.events.push(ToSwarm::GenerateEvent(XStreamEvent::StreamClosed {
            peer_id,
            stream_id,
        }));
    }

    /// Closes a stream
    pub async fn close_stream(
        &mut self,
        peer_id: PeerId,
        stream_id: u128,
    ) -> Result<(), std::io::Error> {
        debug!("Closing stream {} with peer {}", stream_id, peer_id);
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            // The stream will send notification via the closure_notifier channel
            let result = stream.close().await;
            debug!("Stream.close() result for stream {}: {:?}", stream_id, result);
            return result;
        }
        Ok(())
    }

    /// Sends data to the specified stream
    pub async fn send_data(
        &mut self,
        peer_id: PeerId,
        stream_id: u128,
        data: Vec<u8>,
    ) -> Result<(), std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.write_all(data).await;
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Stream not found",
        ))
    }

    /// Reads data from the specified stream
    pub async fn read_data(
        &mut self,
        peer_id: PeerId,
        stream_id: u128,
    ) -> Result<Vec<u8>, std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.read().await;
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Stream not found",
        ))
    }

    /// Reads exact number of bytes from the specified stream
    pub async fn read_exact(
        &mut self,
        peer_id: PeerId,
        stream_id: u128,
        size: usize,
    ) -> Result<Vec<u8>, std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.read_exact(size).await;
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Stream not found",
        ))
    }

    /// Reads all data from the specified stream to the end
    pub async fn read_to_end(
        &mut self,
        peer_id: PeerId,
        stream_id: u128,
    ) -> Result<Vec<u8>, std::io::Error> {
        if let Some(stream) = self.streams.get_mut(&(peer_id, stream_id)) {
            return stream.read_to_end().await;
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Stream not found",
        ))
    }
}

impl NetworkBehaviour for XStreamNetworkBehaviour {
    type ConnectionHandler = XStreamHandler;
    type ToSwarm = XStreamEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let mut handler = XStreamHandler::new();
        // Set peer ID in handler immediately
        handler.set_peer_id(peer);
        Ok(handler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
        _port_use: PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let mut handler = XStreamHandler::new();
        // Set peer ID in handler immediately
        handler.set_peer_id(peer);
        Ok(handler)
    }

    fn on_swarm_event(&mut self, _event: libp2p::swarm::FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        _connection_id: ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        match event {
            XStreamHandlerEvent::IncomingStreamEstablished { stream_id, mut stream } => {
                // Add the stream with its closure notifier
                debug!("Adding stream {} with peer {} and setting closure notifier", stream_id, peer_id);
        
                // Set the closure notifier
                stream.set_closure_notifier(self.closure_sender.clone());
                
                debug!("Closure notifier set for stream {}", stream_id);
                self.streams.insert((peer_id, stream_id), stream.clone());
                
                // Send event about the new stream
                self.events
                    .push(ToSwarm::GenerateEvent(XStreamEvent::IncomingStream {
                        stream: stream.clone(),
                    }));
            },
            XStreamHandlerEvent::OutboundStreamEstablished { stream_id, mut stream } => {
                // Add the stream with its closure notifier
                debug!("Adding stream {} with peer {} and setting closure notifier", stream_id, peer_id);
        
                // Set the closure notifier
                stream.set_closure_notifier(self.closure_sender.clone());
                
                debug!("Closure notifier set for stream {}", stream_id);
                self.streams.insert((peer_id, stream_id), stream.clone());
                
                // Check if there's a waiting sender for this peer
                if let Some(sender) = self.pending_streams.remove(&peer_id) {
                    // Send successful result
                    let _ = sender.send(Ok(stream.clone()));
                }
                
                // Also send StreamEstablished event for backward compatibility
                self.events
                    .push(ToSwarm::GenerateEvent(XStreamEvent::StreamEstablished {
                        peer_id,
                        stream_id,
                    }));
            },

            XStreamHandlerEvent::StreamError { stream_id, error } => {
                // If stream_id is known, remove the stream from HashMap
                if let Some(stream_id) = stream_id {
                    self.streams.remove(&(peer_id, stream_id));
                }

                // Send error event
                self.events
                    .push(ToSwarm::GenerateEvent(XStreamEvent::StreamError {
                        peer_id,
                        stream_id,
                        error,
                    }));
            }
            XStreamHandlerEvent::StreamClosed { stream_id } => {
                debug!("Handler reported stream closed: {}", stream_id);
                // Remove the stream from HashMap
                self.streams.remove(&(peer_id, stream_id));

                // Send stream closed event
                self.events
                    .push(ToSwarm::GenerateEvent(XStreamEvent::StreamClosed {
                        peer_id,
                        stream_id,
                    }));
            }
        }
    }
    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        info!("[POLL] Polling XStreamNetworkBehaviour");
        
        // First check for events from the dedicated closure task
        match self.stream_close_events.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                if let XStreamEvent::StreamClosed { peer_id, stream_id } = &event {
                    info!("[POLL] Received dedicated task closure notification for stream {} from peer {}", stream_id, peer_id);
                    
                    // Remove the stream from the map if it still exists
                    if self.streams.remove(&(*peer_id, *stream_id)).is_some() {
                        info!("[POLL] Stream {} removed from map", stream_id);
                    } else {
                        info!("[POLL] Stream {} was already removed from map", stream_id);
                    }
                }
                
                // Return the event immediately
                info!("[POLL] Returning StreamClosed event from dedicated task");
                return Poll::Ready(ToSwarm::GenerateEvent(event));
            },
            Poll::Ready(None) => {
                error!("[POLL] Stream close events channel closed unexpectedly");
            },
            Poll::Pending => {
                // No events from the dedicated task, continue
                info!("[POLL] No events from dedicated closure task");
            }
        }
        
        // Check for regular events
        if let Some(event) = self.events.pop() {
            info!("[POLL] Returning event from queue: {:?}", event);
            return Poll::Ready(event);
        }
        
        info!("[POLL] No events to process, returning Pending");
        Poll::Pending
    }
}
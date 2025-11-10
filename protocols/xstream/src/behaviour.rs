use super::consts::XSTREAM_PROTOCOL;
use super::types::{SubstreamRole, XStreamDirection, XStreamID, XStreamIDIterator};
use futures::AsyncReadExt;
use libp2p::{
    core::{transport::PortUse, Endpoint},
    swarm::{
        ConnectionDenied, ConnectionHandlerEvent, ConnectionId, FromSwarm, NetworkBehaviour,
        NotifyHandler, ToSwarm,
    },
    Multiaddr, PeerId, Stream, StreamProtocol,
};
use std::collections::HashMap;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, trace, warn};

use super::events::{XStreamEvent, IncomingConnectionApprovePolicy, InboundUpgradeDecision, EstablishedConnection, StreamOpenDecisionSender};
use super::handler::{XStreamHandler, XStreamHandlerEvent, XStreamHandlerIn};
use super::pending_streams::{
    PendingStreamsEvent, PendingStreamsManager, PendingStreamsMessage, SubstreamError,
    SubstreamsPair,
};
use super::xstream::XStream;

/// NetworkBehaviour for working with XStream
pub struct XStreamNetworkBehaviour {
    /// Mapping (peer_id, stream_id) -> XStream
    streams: HashMap<(PeerId, XStreamID), XStream>,
    /// Events waiting to be processed
    events: Vec<ToSwarm<XStreamEvent, XStreamHandlerIn>>,
    /// Pending stream openings
    pending_outgoing_streams: HashMap<XStreamID, oneshot::Sender<Result<XStream, String>>>,
    /// Channel for stream closure notifications - sender only
    closure_sender: mpsc::UnboundedSender<(PeerId, XStreamID)>,
    /// Receiver for events from the dedicated closure task
    stream_close_events: mpsc::UnboundedReceiver<XStreamEvent>,

    // New fields for PendingStreamsManager
    /// Manager for handling paired streams
    pending_streams_manager: Option<PendingStreamsManager>,
    /// Sender for events to PendingStreamsManager
    pending_streams_event_sender: mpsc::UnboundedSender<PendingStreamsEvent>,
    /// Receiver for messages from PendingStreamsManager
    pending_streams_message_receiver: mpsc::UnboundedReceiver<PendingStreamsMessage>,
    /// Task for running PendingStreamsManager
    pending_streams_manager_task: Option<tokio::task::JoinHandle<()>>,

    /// Политика принятия решений о входящих апгрейдах
    pub incoming_approve_policy: IncomingConnectionApprovePolicy,

    id_iter: XStreamIDIterator,
}

impl XStreamNetworkBehaviour {
    /// Creates a new XStreamNetworkBehaviour с политикой AutoApprove по умолчанию
    pub fn new() -> Self {
        Self::new_with_policy(IncomingConnectionApprovePolicy::AutoApprove)
    }
    
    /// Creates a new XStreamNetworkBehaviour с указанной политикой
    pub fn new_with_policy(policy: IncomingConnectionApprovePolicy) -> Self {
        // Channel for closure notifications
        let (closure_sender, mut closure_receiver) = mpsc::unbounded_channel();

        // Channel for events from dedicated task to behavior
        let (event_sender, stream_close_events) = mpsc::unbounded_channel();

        // Channels for PendingStreamsManager
        let (message_sender, pending_streams_message_receiver) = mpsc::unbounded_channel();
        let mut pending_streams_manager = PendingStreamsManager::new(message_sender);
        let pending_streams_event_sender = pending_streams_manager.get_event_sender();

        tokio::spawn(async move {
            trace!("[CLOSURE_TASK] Started dedicated stream closure monitoring task");

            while let Some((peer_id, stream_id)) = closure_receiver.recv().await {
                trace!(
                    "[CLOSURE_TASK] Received closure notification for stream {:?} from peer {}",
                    stream_id,
                    peer_id
                );

                // Send an event to the behavior
                match event_sender.send(XStreamEvent::StreamClosed {
                    peer_id,
                    stream_id,
                }) {
                    Ok(_) => trace!("[CLOSURE_TASK] Successfully sent StreamClosed event to behavior for stream {:?}", stream_id),
                    Err(e) => error!("[CLOSURE_TASK] Failed to send StreamClosed event: {}", e),
                }
            }

            trace!("[CLOSURE_TASK] Stream closure monitoring task exited - channel closed");
        });

        let mut behaviour = Self {
            streams: HashMap::new(),
            events: Vec::new(),
            pending_outgoing_streams: HashMap::new(),
            closure_sender,
            stream_close_events,

            // Initialize fields for PendingStreamsManager
            pending_streams_manager: Some(pending_streams_manager),
            pending_streams_event_sender,
            pending_streams_message_receiver,
            pending_streams_manager_task: None,
            incoming_approve_policy: policy,
            id_iter: XStreamIDIterator::new(),
        };

        // Start PendingStreamsManager in a separate task
        behaviour.start_pending_streams_manager();

        behaviour
    }

    /// Starts PendingStreamsManager in a separate task
    fn start_pending_streams_manager(&mut self) {
        if let Some(manager) = self.pending_streams_manager.take() {
            let mut pending_manager = manager;

            // Create and save the task
            self.pending_streams_manager_task = Some(tokio::spawn(async move {
                pending_manager.run().await;
            }));

            info!("PendingStreamsManager started in a separate task");
        }
    }

    /// Handles messages from PendingStreamsManager
    fn handle_pending_streams_message(&mut self, message: PendingStreamsMessage) {
        match message {
            PendingStreamsMessage::SubstreamPairReady(pair) => {
                debug!("Received substream pair for key {:?}", pair.key);

                // Create XStream from the received stream pair
                let stream_id = pair.key.stream_id;
                let peer_id = pair.key.peer_id;

                // Split both streams into read and write parts
                let (main_read, main_write) = AsyncReadExt::split(pair.main);
                let (error_read, error_write) = AsyncReadExt::split(pair.error);

                // Create XStream with both main and error streams
                let xstream = XStream::new(
                    stream_id,
                    peer_id,
                    main_read,
                    main_write,
                    error_read,
                    error_write,
                    pair.key.direction,
                    self.closure_sender.clone(),
                );

                // Generate event for new stream
                if pair.key.direction == XStreamDirection::Inbound {
                    self.events
                        .push(ToSwarm::GenerateEvent(XStreamEvent::IncomingStream {
                            stream: xstream,
                        }));
                } else {
                    // Check if there's a waiting sender for this peer
                    if let Some(sender) = self.pending_outgoing_streams.remove(&stream_id) {
                        // Send successful result
                        let _ = sender.send(Ok(xstream));
                    }

                    // Also send StreamEstablished event for backward compatibility
                    self.events
                        .push(ToSwarm::GenerateEvent(XStreamEvent::StreamEstablished {
                            peer_id,
                            stream_id,
                        }));
                }
            }
            PendingStreamsMessage::SubstreamError(error) => {
                match error {
                    SubstreamError::SubstreamTimeoutError { key, role } => {
                        warn!(
                            "Substream timeout error for key {:?} with role {:?}",
                            key, role
                        );

                        // Generate error event
                        self.events
                            .push(ToSwarm::GenerateEvent(XStreamEvent::StreamError {
                                peer_id: key.peer_id,
                                stream_id: Some(key.stream_id),
                                error: format!(
                                    "Timeout waiting for matching substream with role {:?}",
                                    role
                                ),
                            }));
                    }
                    SubstreamError::SubstreamSameRole { key, role } => {
                        error!(
                            "Received streams with same role error: key={:?}, role={:?}",
                            key, role
                        );

                        // Generate error event
                        self.events
                            .push(ToSwarm::GenerateEvent(XStreamEvent::StreamError {
                                peer_id: key.peer_id,
                                stream_id: Some(key.stream_id),
                                error: format!("Received streams with same role: {:?}", role),
                            }));
                    }
                    SubstreamError::SubstreamReadHeaderError {
                        direction,
                        peer_id,
                        connection_id,
                        error,
                    } => {
                        error!(
                            "Error reading stream header: {:?}, direction: {:?}, peer: {}",
                            error, direction, peer_id
                        );

                        // Generate error event
                        self.events
                            .push(ToSwarm::GenerateEvent(XStreamEvent::StreamError {
                                peer_id,
                                stream_id: None,
                                error: format!("Error reading stream header: {}", error),
                            }));
                    }
                }
            }
        }
    }

    /// Requests to open a new stream to the specified peer
    pub fn request_open_stream(&mut self, peer_id: PeerId) -> XStreamID {
        let stream_id = self.id_iter.next().unwrap();
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: XStreamHandlerIn::OpenStreamWithRole {
                stream_id: stream_id,
                role: SubstreamRole::Main,
            },
        });
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: XStreamHandlerIn::OpenStreamWithRole {
                stream_id: stream_id,
                role: SubstreamRole::Error,
            },
        });
        return stream_id;
    }

    /// Asynchronously opens a new stream and returns XStream or an error
    pub async fn open_stream(
        &mut self,
        peer_id: PeerId,
        response: oneshot::Sender<Result<XStream, String>>,
    ) {
        // Request stream opening
        let stream_id = self.request_open_stream(peer_id);
        self.pending_outgoing_streams.insert(stream_id, response);
    }

    /// Handles stream opening errors for specific stream_id
    pub fn handle_stream_open_error(&mut self, stream_id: XStreamID, error: String) {
        if let Some(sender) = self.pending_outgoing_streams.remove(&stream_id) {
            let _ = sender.send(Err(error));
        }
    }

    /// Notifies that a stream is closed
    pub fn notify_stream_closed(&mut self, peer_id: PeerId, stream_id: XStreamID) {
        debug!("Manual notification of stream closure: {:?}", stream_id);
        // Remove the stream from the active streams map
        self.streams.remove(&(peer_id, stream_id));
        // Generate the appropriate event
        self.events
            .push(ToSwarm::GenerateEvent(XStreamEvent::StreamClosed {
                peer_id,
                stream_id,
            }));
    }



}

// Need to add new variants to XStreamHandlerEvent to handle raw streams
#[derive(Debug)]
pub enum ExtendedXStreamHandlerEvent {
    /// Original events
    Original(XStreamHandlerEvent),
    /// New incoming raw stream (without processing)
    IncomingStreamRaw {
        /// libp2p Stream
        stream: Stream,
        /// Protocol
        protocol: StreamProtocol,
    },
    /// New outbound raw stream (without processing)
    OutboundStreamRaw {
        /// libp2p Stream
        stream: Stream,
        /// Protocol
        protocol: StreamProtocol,
    },
}

impl NetworkBehaviour for XStreamNetworkBehaviour {
    type ConnectionHandler = XStreamHandler;
    type ToSwarm = XStreamEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let established_connection = EstablishedConnection::Inbound {
            remote_addr: remote_addr.clone(),
            local_addr: local_addr.clone(),
        };
        
        let mut handler = XStreamHandler::new(connection_id, peer, established_connection);
        // Set peer ID in handler immediately
        //handler.set_peer_id(peer);
        // Provide closure sender to the handler
        handler.set_closure_sender(self.closure_sender.clone());
        Ok(handler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
        port_use: PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        let established_connection = EstablishedConnection::Outbound {
            addr: addr.clone(),
        };
        
        let mut handler = XStreamHandler::new(connection_id, peer, established_connection);
        // Set peer ID in handler immediately
        handler.set_peer_id(peer);
        // Provide closure sender to the handler
        handler.set_closure_sender(self.closure_sender.clone());
        Ok(handler)
    }

    fn on_swarm_event(&mut self, _event: libp2p::swarm::FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        // We need to adapt the handler to work with our new raw stream approach
        // This is a simplified version - in a real implementation, you would likely
        // modify the XStreamHandlerEvent to include the new variants
        match event {
            XStreamHandlerEvent::IncomingStreamEstablished { stream } => {
                println!("INCOMING");
                let direction = XStreamDirection::Inbound;
                if let Err(e) =
                    self.pending_streams_event_sender
                        .send(PendingStreamsEvent::NewStream {
                            stream,
                            direction,
                            peer_id,
                            connection_id,
                            role: SubstreamRole::Main,     // can be any
                            xstreamid: XStreamID::from(0), // incoming
                        })
                {
                    error!("Failed to send stream to PendingStreamsManager: {}", e);
                }
            }
            XStreamHandlerEvent::OutboundStreamEstablished {
                role,
                stream_id,
                stream,
            } => {
                let direction = XStreamDirection::Outbound;
                println!("OUTBOUND");
                if let Err(e) =
                    self.pending_streams_event_sender
                        .send(PendingStreamsEvent::NewStream {
                            stream,
                            direction,
                            peer_id,
                            connection_id,
                            role,
                            xstreamid: stream_id,
                        })
                {
                    error!("Failed to send stream to PendingStreamsManager: {}", e);
                }
            }
            XStreamHandlerEvent::StreamError { stream_id, error } => {
                // If stream_id is known, remove the stream from HashMap
                if let Some(stream_id) = stream_id {
                    self.streams.remove(&(peer_id, stream_id));
                    // Also handle stream opening errors for pending requests
                    self.handle_stream_open_error(stream_id, error.clone());
                } else {
                    // If stream_id is None, this might be an error from swarm_handler rejecting an incoming stream
                    // We need to find and fail any pending outgoing streams to this peer
                    let pending_stream_ids: Vec<XStreamID> = self.pending_outgoing_streams
                        .keys()
                        .cloned()
                        .collect();
                    
                    for stream_id in pending_stream_ids {
                        // For now, we'll fail all pending streams to this peer
                        // In a more sophisticated implementation, we might want to track which stream_id corresponds to which peer
                        self.handle_stream_open_error(stream_id, error.clone());
                    }
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
                debug!("Handler reported stream closed: {:?}", stream_id);
                // Remove the stream from HashMap
                self.streams.remove(&(peer_id, stream_id));

                // Send stream closed event
                self.events
                    .push(ToSwarm::GenerateEvent(XStreamEvent::StreamClosed {
                        peer_id,
                        stream_id,
                    }));
            }
            XStreamHandlerEvent::IncomingStreamRequest { peer_id, connection_id, decision_sender } => {
                match self.incoming_approve_policy {
                    IncomingConnectionApprovePolicy::AutoApprove => {
                        // Автоматически одобряем без генерации события
                        let _ = decision_sender.approve();
                    }
                    IncomingConnectionApprovePolicy::ApproveViaEvent => {
                        // Генерируем событие для пользовательской обработки
                        self.events.push(ToSwarm::GenerateEvent(
                            XStreamEvent::IncomingStreamRequest {
                                peer_id,
                                connection_id,
                                decision_sender,
                            }
                        ));
                    }
                }
            } // In a real implementation, you'd have the following additional cases:
              // XStreamHandlerEvent::IncomingStreamRaw { stream, protocol } => {
              //     // Pass raw stream to PendingStreamsManager
              //     self.handle_new_stream(
              //         stream,
              //         XStreamDirection::Inbound,
              //         peer_id,
              //         connection_id
              //     );
              // },
              // XStreamHandlerEvent::OutboundStreamRaw { stream, protocol } => {
              //     // Pass raw stream to PendingStreamsManager
              //     self.handle_new_stream(
              //         stream,
              //         XStreamDirection::Outbound,
              //         peer_id,
              //         connection_id
              //     );
              // },
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        trace!("[POLL] Polling XStreamNetworkBehaviour");

        // First check for messages from PendingStreamsManager
        match self.pending_streams_message_receiver.poll_recv(cx) {
            Poll::Ready(Some(message)) => {
                trace!("[POLL] Received message from PendingStreamsManager");
                self.handle_pending_streams_message(message);

                // If events were generated during message handling, return one
                if let Some(event) = self.events.pop() {
                    return Poll::Ready(event);
                }
            }
            Poll::Ready(None) => {
                error!("[POLL] PendingStreams message channel closed unexpectedly");
            }
            Poll::Pending => {
                // No messages from PendingStreamsManager, continue
                trace!("[POLL] No messages from PendingStreamsManager");
            }
        }

        // Check for events from the dedicated closure task
        match self.stream_close_events.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                if let XStreamEvent::StreamClosed { peer_id, stream_id } = &event {
                    trace!("[POLL] Received dedicated task closure notification for stream {:?} from peer {}", stream_id, peer_id);

                    // Remove the stream from the map if it still exists
                    if self.streams.remove(&(*peer_id, *stream_id)).is_some() {
                        trace!("[POLL] Stream {:?} removed from map", stream_id);
                    } else {
                        trace!("[POLL] Stream {:?} was already removed from map", stream_id);
                    }
                }

                // Return the event immediately
                trace!("[POLL] Returning StreamClosed event from dedicated task");
                return Poll::Ready(ToSwarm::GenerateEvent(event));
            }
            Poll::Ready(None) => {
                error!("[POLL] Stream close events channel closed unexpectedly");
            }
            Poll::Pending => {
                // No events from the dedicated task, continue
                trace!("[POLL] No events from dedicated closure task");
            }
        }

        // Check for regular events
        if let Some(event) = self.events.pop() {
            trace!("[POLL] Returning event from queue: {:?}", event);
            return Poll::Ready(event);
        }

        trace!("[POLL] No events to process, returning Pending");
        Poll::Pending
    }
}

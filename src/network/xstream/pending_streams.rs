use super::header::{read_header, XStreamHeader};
use super::types::{SubstreamRole, XStreamDirection, XStreamID};
use futures::AsyncReadExt;
use futures::AsyncWriteExt; // Added for close() method
use libp2p::swarm::ConnectionId; // Correct import for ConnectionId
use libp2p::{PeerId, Stream};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

// Represents a key for identifying and matching substreams
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstreamKey {
    pub direction: XStreamDirection,
    pub peer_id: PeerId,
    pub connection_id: ConnectionId,
    pub stream_id: XStreamID,
}

// Manually implement Hash for SubstreamKey since XStreamDirection doesn't implement Hash
impl std::hash::Hash for SubstreamKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the direction as a discriminant value
        (self.direction as u8).hash(state);
        self.peer_id.hash(state);
        self.connection_id.hash(state);
        self.stream_id.hash(state);
    }
}

impl SubstreamKey {
    pub fn new(
        direction: XStreamDirection,
        peer_id: PeerId,
        connection_id: ConnectionId,
        stream_id: XStreamID,
    ) -> Self {
        Self {
            direction,
            peer_id,
            connection_id,
            stream_id,
        }
    }
}

// A pair of substreams (main and error)
#[derive(Debug)]
pub struct SubstreamsPair {
    pub key: SubstreamKey,
    pub main: Stream,
    pub error: Stream,
}

// Events that can be sent to the PendingStreamsManager
#[derive(Debug)]
pub enum PendingStreamsEvent {
    NewStream {
        stream: Stream,
        direction: XStreamDirection,
        peer_id: PeerId,
        connection_id: ConnectionId,
        role: SubstreamRole,
        xstreamid: XStreamID, //for outgoing only
    },
    CleanupTimeouts,
}

// Messages that the PendingStreamsManager can send back
#[derive(Debug)]
pub enum PendingStreamsMessage {
    SubstreamPairReady(SubstreamsPair),
    SubstreamError(SubstreamError),
}

// Different types of errors that can occur
#[derive(Debug)]
pub enum SubstreamError {
    SubstreamTimeoutError {
        key: SubstreamKey,
        role: SubstreamRole,
    },
    SubstreamSameRole {
        key: SubstreamKey,
        role: SubstreamRole,
    },
    SubstreamReadHeaderError {
        direction: XStreamDirection,
        peer_id: PeerId,
        connection_id: ConnectionId,
        error: std::io::Error,
    },
}

// A struct to hold a pending stream and its metadata
struct PendingStream {
    stream: Stream,
    role: SubstreamRole,
    timestamp: Instant,
}

// The main manager for handling pending streams
pub struct PendingStreamsManager {
    // Map of keys to pending streams
    pending_streams: HashMap<SubstreamKey, PendingStream>,
    // Time to wait for a matching stream
    timeout_duration: Duration,
    // Sender for events to the manager
    event_sender: mpsc::UnboundedSender<PendingStreamsEvent>,
    // Receiver for events to the manager
    event_receiver: mpsc::UnboundedReceiver<PendingStreamsEvent>,
    // Sender for messages from the manager
    message_sender: mpsc::UnboundedSender<PendingStreamsMessage>,
    // Set of keys that need cleanup
    streams_to_cleanup: HashSet<SubstreamKey>,
}

impl PendingStreamsManager {
    pub fn new(message_sender: mpsc::UnboundedSender<PendingStreamsMessage>) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            pending_streams: HashMap::new(),
            timeout_duration: Duration::from_secs(15), // Default timeout
            event_sender,
            event_receiver,
            message_sender,
            streams_to_cleanup: HashSet::new(),
        }
    }

    // Get the sender for events to the manager
    pub fn get_event_sender(&self) -> mpsc::UnboundedSender<PendingStreamsEvent> {
        self.event_sender.clone()
    }

    // Set the timeout duration
    pub fn set_timeout_duration(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }

    // Start the manager's processing loop
    pub async fn run(&mut self) {
        info!("PendingStreamsManager started");

        // Schedule periodic cleanup task
        let event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = event_sender.send(PendingStreamsEvent::CleanupTimeouts) {
                    error!("Failed to send cleanup event: {}", e);
                    break;
                }
            }
        });

        // Main event processing loop
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                PendingStreamsEvent::NewStream {
                    stream,
                    direction,
                    peer_id,
                    connection_id,
                    role,
                    xstreamid,
                } => {
                    self.handle_substream(
                        stream,
                        direction,
                        peer_id,
                        connection_id,
                        role,
                        xstreamid,
                    )
                    .await;
                }
                PendingStreamsEvent::CleanupTimeouts => {
                    self.cleanup_timeouts();
                }
            }
        }

        info!("PendingStreamsManager stopped");
    }

    // Handle a new substream

    async fn handle_substream(
        &mut self,
        mut stream: Stream,
        direction: XStreamDirection,
        peer_id: PeerId,
        connection_id: ConnectionId,
        role: SubstreamRole,
        stream_id: XStreamID,
    ) {
        let key: SubstreamKey;
        let actual_role: SubstreamRole;

        // TODO: wrap it into async move!!!
        if direction == XStreamDirection::Inbound {
            //read the header!!!!
            // Split the stream to read the header
            let (mut read, write) = AsyncReadExt::split(stream);

            // Try to read the header with timeout
            let header_result = match timeout(self.timeout_duration, read_header(&mut read)).await {
                Ok(result) => result,
                Err(_) => {
                    error!("Timeout reading header from stream");
                    let _ = self
                        .message_sender
                        .send(PendingStreamsMessage::SubstreamError(
                            SubstreamError::SubstreamReadHeaderError {
                                direction,
                                peer_id,
                                connection_id,
                                error: std::io::Error::new(
                                    std::io::ErrorKind::TimedOut,
                                    "Timeout reading header",
                                ),
                            },
                        ));
                    return;
                }
            };

            // Check if header was successfully read
            let header = match header_result {
                Ok(header) => header,
                Err(e) => {
                    error!("Error reading header: {:?}", e);
                    let _ = self
                        .message_sender
                        .send(PendingStreamsMessage::SubstreamError(
                            SubstreamError::SubstreamReadHeaderError {
                                direction,
                                peer_id,
                                connection_id,
                                error: e,
                            },
                        ));
                    return;
                }
            };

            // Reconstruct the stream by joining read and write parts
            stream = match read.reunite(write) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to reunite stream parts: {:?}", e);
                    return;
                }
            };

            // Create a key for this stream
            key = SubstreamKey::new(direction, peer_id, connection_id, header.stream_id);
            actual_role = header.stream_type
        } else {
            key = SubstreamKey::new(direction, peer_id, connection_id, stream_id);
            actual_role = role
        }

        debug!(
            "Received stream with key {:?} and role {:?}",
            key, actual_role
        );

        // Check if we already have a pending stream with the same key
        if let Some(mut pending) = self.pending_streams.remove(&key) {
            // We have a matching stream, check roles
            if pending.role == actual_role {
                // Both streams have the same role, this is an error
                error!("Received streams with same role: {:?}", actual_role);

                // Try to close both streams
                // Use AsyncWriteExt::close method
                let _ = AsyncWriteExt::close(&mut stream).await;
                let _ = AsyncWriteExt::close(&mut pending.stream).await;

                // Send error message
                let _ = self
                    .message_sender
                    .send(PendingStreamsMessage::SubstreamError(
                        SubstreamError::SubstreamSameRole {
                            key,
                            role: actual_role,
                        },
                    ));
                return;
            }

            // Roles are different, create a pair
            let (main_stream, error_stream) = if actual_role == SubstreamRole::Main {
                (stream, pending.stream)
            } else {
                (pending.stream, stream)
            };

            // Create the pair and send it
            let pair = SubstreamsPair {
                key: key.clone(),
                main: main_stream,
                error: error_stream,
            };

            info!("Created substream pair for {:?}", key);
            let _ = self
                .message_sender
                .send(PendingStreamsMessage::SubstreamPairReady(pair));
        } else {
            // No matching stream yet, store it as pending
            self.pending_streams.insert(
                key.clone(),
                PendingStream {
                    stream,
                    role: actual_role,
                    timestamp: Instant::now(),
                },
            );
            debug!(
                "Stored pending stream with key {:?} and role {:?}",
                key, actual_role
            );
        }
    }

    // Clean up streams that have been waiting too long
    fn cleanup_timeouts(&mut self) {
        let now = Instant::now();
        self.streams_to_cleanup.clear();

        // Find streams to clean up
        for (key, pending) in &self.pending_streams {
            if now.duration_since(pending.timestamp) > self.timeout_duration {
                self.streams_to_cleanup.insert(key.clone());
            }
        }

        // Process the streams that need cleanup
        for key in &self.streams_to_cleanup {
            if let Some(mut pending) = self.pending_streams.remove(key) {
                // Try to close the stream
                tokio::spawn(async move {
                    let _ = AsyncWriteExt::close(&mut pending.stream).await;
                });

                // Send timeout error message
                let _ = self
                    .message_sender
                    .send(PendingStreamsMessage::SubstreamError(
                        SubstreamError::SubstreamTimeoutError {
                            key: key.clone(),
                            role: pending.role,
                        },
                    ));

                debug!("Cleaned up timed out stream with key {:?}", key);
            }
        }
    }
}

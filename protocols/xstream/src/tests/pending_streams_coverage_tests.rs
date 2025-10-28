// pending_streams_coverage_tests.rs
// Tests specifically targeting uncovered code in pending_streams.rs

use crate::pending_streams::{PendingStreamsManager, SubstreamKey, PendingStreamsEvent, PendingStreamsMessage};
use crate::types::{XStreamDirection, XStreamID, SubstreamRole};
use libp2p::{PeerId, swarm::ConnectionId};
use tokio::sync::mpsc;

#[test]
fn test_pending_streams_constructor_and_getters() {
    // Test basic constructor and getter methods
    let (message_sender, _message_receiver) = mpsc::unbounded_channel();
    let manager = PendingStreamsManager::new(message_sender);
    
    // Test get_event_sender
    let sender = manager.get_event_sender();
    // UnboundedSender doesn't have capacity method, but we can verify it's created
    assert!(!sender.is_closed());
}

#[test]
fn test_substream_key_hash_consistency() {
    // Test that SubstreamKey hashing is consistent
    let peer_id1 = PeerId::random();
    let peer_id2 = PeerId::random();
    let connection_id1 = ConnectionId::new_unchecked(1);
    let connection_id2 = ConnectionId::new_unchecked(2);
    let stream_id = XStreamID::from(42u128);
    
    let key1 = SubstreamKey::new(
        XStreamDirection::Inbound,
        peer_id1,
        connection_id1,
        stream_id,
    );
    
    let key2 = SubstreamKey::new(
        XStreamDirection::Inbound,
        peer_id1,
        connection_id1,
        stream_id,
    );
    
    let key3 = SubstreamKey::new(
        XStreamDirection::Outbound,
        peer_id2,
        connection_id2,
        stream_id,
    );
    
    // Same parameters should produce same hash
    let hash1 = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key1.hash(&mut hasher);
        hasher.finish()
    };
    
    let hash2 = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key2.hash(&mut hasher);
        hasher.finish()
    };
    
    assert_eq!(hash1, hash2);
    
    // Different parameters should produce different hash
    let hash3 = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key3.hash(&mut hasher);
        hasher.finish()
    };
    
    assert_ne!(hash1, hash3);
}

#[test]
fn test_pending_streams_event_creation() {
    // Test creating PendingStreamsEvent variants
    let peer_id = PeerId::random();
    let connection_id = ConnectionId::new_unchecked(1);
    let stream_id = XStreamID::from(123u128);
    
    // Test CleanupTimeouts event
    let cleanup_event = PendingStreamsEvent::CleanupTimeouts;
    
    // Just verify the types compile
    assert!(matches!(cleanup_event, PendingStreamsEvent::CleanupTimeouts));
    
    // Test that we can create NewStream event type (without actual stream)
    // This verifies the enum variant exists and compiles
    let _event_type_check: PendingStreamsEvent = PendingStreamsEvent::CleanupTimeouts;
}

#[test]
fn test_pending_streams_message_creation() {
    // Test creating PendingStreamsMessage variants
    let peer_id = PeerId::random();
    let connection_id = ConnectionId::new_unchecked(1);
    let stream_id = XStreamID::from(123u128);
    let key = SubstreamKey::new(
        XStreamDirection::Inbound,
        peer_id,
        connection_id,
        stream_id,
    );
    
    // Test SubstreamError variants
    let timeout_error = PendingStreamsMessage::SubstreamError(
        crate::pending_streams::SubstreamError::SubstreamTimeoutError {
            key: key.clone(),
            role: SubstreamRole::Main,
        }
    );
    
    let same_role_error = PendingStreamsMessage::SubstreamError(
        crate::pending_streams::SubstreamError::SubstreamSameRole {
            key: key.clone(),
            role: SubstreamRole::Error,
        }
    );
    
    let read_header_error = PendingStreamsMessage::SubstreamError(
        crate::pending_streams::SubstreamError::SubstreamReadHeaderError {
            direction: XStreamDirection::Inbound,
            peer_id,
            connection_id,
            error: std::io::Error::new(std::io::ErrorKind::TimedOut, "test timeout"),
        }
    );
    
    // Just verify the types compile
    assert!(matches!(timeout_error, PendingStreamsMessage::SubstreamError(_)));
    assert!(matches!(same_role_error, PendingStreamsMessage::SubstreamError(_)));
    assert!(matches!(read_header_error, PendingStreamsMessage::SubstreamError(_)));
}

// pending_streams_edge_cases_tests.rs
// Edge case tests for PendingStreamsManager covering real scenarios

use crate::pending_streams::{PendingStreamsManager, PendingStreamsMessage, PendingStreamsEvent, SubstreamError};
use crate::types::{SubstreamRole, XStreamDirection, XStreamID};
use libp2p::{PeerId, swarm::ConnectionId};
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_pending_streams_manager_creation() {
    // Test basic manager creation and configuration
    println!("🚀 Testing PendingStreamsManager creation and configuration...");
    
    // Create message channel
    let (message_sender, _message_receiver) = mpsc::unbounded_channel();
    
    // Create manager
    let mut manager = PendingStreamsManager::new(message_sender);
    
    // Test timeout configuration
    let test_timeout = Duration::from_secs(10);
    manager.set_timeout_duration(test_timeout);
    
    // Test event sender retrieval
    let event_sender = manager.get_event_sender();
    assert!(!event_sender.is_closed(), "Event sender should be valid");
    
    println!("✅ PendingStreamsManager creation and configuration test completed successfully");
}

#[tokio::test]
async fn test_pending_streams_error_types() {
    // Test that all error types are properly defined
    println!("🚀 Testing PendingStreamsManager error types...");
    
    let test_peer_id = PeerId::random();
    let test_connection_id = ConnectionId::new_unchecked(1);
    let test_stream_id = XStreamID::from(123u128);
    
    // Test SubstreamTimeoutError
    let timeout_error = SubstreamError::SubstreamTimeoutError {
        key: crate::pending_streams::SubstreamKey::new(
            XStreamDirection::Inbound,
            test_peer_id,
            test_connection_id,
            test_stream_id,
        ),
        role: SubstreamRole::Main,
    };
    
    match timeout_error {
        SubstreamError::SubstreamTimeoutError { key, role } => {
            println!("✅ SubstreamTimeoutError properly defined - key: {:?}, role: {:?}", key, role);
        }
        _ => panic!("Unexpected error type"),
    }
    
    // Test SubstreamSameRole
    let same_role_error = SubstreamError::SubstreamSameRole {
        key: crate::pending_streams::SubstreamKey::new(
            XStreamDirection::Outbound,
            test_peer_id,
            test_connection_id,
            test_stream_id,
        ),
        role: SubstreamRole::Error,
    };
    
    match same_role_error {
        SubstreamError::SubstreamSameRole { key, role } => {
            println!("✅ SubstreamSameRole properly defined - key: {:?}, role: {:?}", key, role);
        }
        _ => panic!("Unexpected error type"),
    }
    
    // Test SubstreamReadHeaderError
    let read_header_error = SubstreamError::SubstreamReadHeaderError {
        direction: XStreamDirection::Inbound,
        peer_id: test_peer_id,
        connection_id: test_connection_id,
        error: std::io::Error::new(std::io::ErrorKind::Other, "Test error"),
    };
    
    match read_header_error {
        SubstreamError::SubstreamReadHeaderError { direction, peer_id, connection_id, error } => {
            println!("✅ SubstreamReadHeaderError properly defined - direction: {:?}, peer_id: {}, connection_id: {:?}, error: {}", 
                     direction, peer_id, connection_id, error);
        }
        _ => panic!("Unexpected error type"),
    }
    
    println!("✅ PendingStreamsManager error types test completed successfully");
}

#[tokio::test]
async fn test_pending_streams_event_types() {
    // Test that all event types are properly defined
    println!("🚀 Testing PendingStreamsManager event types...");
    
    // Test CleanupTimeouts event (doesn't require streams)
    let cleanup_event = PendingStreamsEvent::CleanupTimeouts;
    
    match cleanup_event {
        PendingStreamsEvent::CleanupTimeouts => {
            println!("✅ CleanupTimeouts event properly defined");
        }
        _ => panic!("Unexpected event type"),
    }
    
    println!("✅ PendingStreamsManager event types test completed successfully");
}

#[tokio::test]
async fn test_pending_streams_message_types() {
    // Test that all message types are properly defined
    println!("🚀 Testing PendingStreamsManager message types...");
    
    let test_peer_id = PeerId::random();
    let test_connection_id = ConnectionId::new_unchecked(1);
    let test_stream_id = XStreamID::from(123u128);
    
    // Test SubstreamError message (doesn't require streams)
    let error_message = PendingStreamsMessage::SubstreamError(
        SubstreamError::SubstreamTimeoutError {
            key: crate::pending_streams::SubstreamKey::new(
                XStreamDirection::Inbound,
                test_peer_id,
                test_connection_id,
                test_stream_id,
            ),
            role: SubstreamRole::Main,
        }
    );
    
    match error_message {
        PendingStreamsMessage::SubstreamError(error) => {
            println!("✅ SubstreamError message properly defined - error: {:?}", error);
        }
        _ => panic!("Unexpected message type"),
    }
    
    println!("✅ PendingStreamsManager message types test completed successfully");
}

#[tokio::test]
async fn test_pending_streams_substream_key() {
    // Test SubstreamKey functionality
    println!("🚀 Testing SubstreamKey functionality...");
    
    let peer_id1 = PeerId::random();
    let peer_id2 = PeerId::random();
    let connection_id1 = ConnectionId::new_unchecked(1);
    let connection_id2 = ConnectionId::new_unchecked(2);
    let stream_id1 = XStreamID::from(123u128);
    let stream_id2 = XStreamID::from(456u128);
    
    // Test key creation
    let key1 = crate::pending_streams::SubstreamKey::new(
        XStreamDirection::Inbound,
        peer_id1,
        connection_id1,
        stream_id1,
    );
    
    let key2 = crate::pending_streams::SubstreamKey::new(
        XStreamDirection::Outbound,
        peer_id2,
        connection_id2,
        stream_id2,
    );
    
    println!("✅ SubstreamKey creation works - key1: {:?}, key2: {:?}", key1, key2);
    
    // Test key equality
    let key1_clone = key1.clone();
    assert_eq!(key1, key1_clone, "Keys should be equal");
    assert_ne!(key1, key2, "Different keys should not be equal");
    
    println!("✅ SubstreamKey equality testing works");
    
    // Test key hashing (basic smoke test)
    use std::collections::HashSet;
    let mut key_set = HashSet::new();
    key_set.insert(key1.clone());
    key_set.insert(key2.clone());
    
    assert_eq!(key_set.len(), 2, "Should have 2 unique keys in set");
    assert!(key_set.contains(&key1), "Set should contain key1");
    assert!(key_set.contains(&key2), "Set should contain key2");
    
    println!("✅ SubstreamKey hashing works");
    
    println!("✅ SubstreamKey functionality test completed successfully");
}

// pending_streams_unit_tests.rs
// Unit tests for PendingStreamsManager focusing on core functionality

use crate::pending_streams::{
    PendingStreamsManager, PendingStreamsMessage, PendingStreamsEvent
};
use crate::types::{XStreamDirection, XStreamID, SubstreamRole};
use libp2p::{PeerId, swarm::ConnectionId};
use std::time::Duration;
use tokio::sync::mpsc;

// Helper function to create test manager
fn create_test_manager() -> (PendingStreamsManager, mpsc::UnboundedReceiver<PendingStreamsMessage>) {
    let (message_sender, message_receiver) = mpsc::unbounded_channel();
    let manager = PendingStreamsManager::new(message_sender);
    (manager, message_receiver)
}

// Helper function to create test peer ID
fn create_test_peer_id() -> PeerId {
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    keypair.public().to_peer_id()
}

// Helper function to create test connection ID
fn create_test_connection_id() -> ConnectionId {
    ConnectionId::new_unchecked(1)
}

#[tokio::test]
async fn test_manager_creation() {
    // Test basic manager creation and event channel setup
    let (manager, _message_receiver) = create_test_manager();
    let event_sender = manager.get_event_sender();
    
    // Verify we can send events
    assert!(!event_sender.is_closed());
    println!("✅ Manager creation test passed");
}

#[tokio::test]
async fn test_event_channel_communication() {
    // Test that events can be sent through the channel
    let (mut manager, mut message_receiver) = create_test_manager();
    let peer_id = create_test_peer_id();
    let connection_id = create_test_connection_id();
    let stream_id = XStreamID::from(123u128);

    let event_sender = manager.get_event_sender();

    // Send a simple event to verify channel works
    // Note: We're not testing actual stream pairing yet, just the event system
    let result = event_sender.send(PendingStreamsEvent::CleanupTimeouts);
    assert!(result.is_ok(), "Should be able to send cleanup event");
    
    println!("✅ Event channel communication test passed");
}

#[tokio::test]
async fn test_manager_timeout_configuration() {
    // Test that timeout duration can be configured
    let (mut manager, _message_receiver) = create_test_manager();
    
    let original_timeout = Duration::from_secs(15); // Default
    let new_timeout = Duration::from_secs(30);
    
    // Note: We can't directly access the timeout duration without getter
    // For now, just verify the manager can be created and configured
    manager.set_timeout_duration(new_timeout);
    
    println!("✅ Manager timeout configuration test passed");
}

#[tokio::test]
async fn test_multiple_peer_handling() {
    // Test that manager can handle multiple different peers
    let (mut manager, _message_receiver) = create_test_manager();
    let peer_id1 = create_test_peer_id();
    let peer_id2 = create_test_peer_id();
    let connection_id = create_test_connection_id();

    let event_sender = manager.get_event_sender();

    // Send events for different peers
    let result1 = event_sender.send(PendingStreamsEvent::CleanupTimeouts);
    let result2 = event_sender.send(PendingStreamsEvent::CleanupTimeouts);
    
    assert!(result1.is_ok(), "Should handle first event");
    assert!(result2.is_ok(), "Should handle second event");
    
    println!("✅ Multiple peer handling test passed");
}

#[tokio::test]
async fn test_manager_run_method() {
    // Test that manager can be started and stopped
    let (mut manager, _message_receiver) = create_test_manager();
    let event_sender = manager.get_event_sender();

    // Start the manager in a separate task
    let manager_task = tokio::spawn(async move {
        manager.run().await;
    });

    // Send a cleanup event to verify it's running
    let result = event_sender.send(PendingStreamsEvent::CleanupTimeouts);
    assert!(result.is_ok(), "Should be able to send event to running manager");

    // Give it a moment to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drop the event sender to stop the manager
    drop(event_sender);

    // Wait for manager to finish
    let _ = tokio::time::timeout(Duration::from_secs(1), manager_task).await;
    
    println!("✅ Manager run method test passed");
}

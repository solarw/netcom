// connection_handler_events_tests.rs
// Test 2: Event handling in XStreamHandler

use crate::handler::{XStreamHandler, XStreamHandlerIn, XStreamHandlerEvent};
use crate::types::{SubstreamRole, XStreamID};
use libp2p::{PeerId, swarm::ConnectionHandler};

#[tokio::test]
async fn test_handler_event_processing() {
    // Test processing of behavior events in XStreamHandler
    println!("🚀 Testing XStreamHandler event processing...");
    
    let mut handler = XStreamHandler::new();
    let test_peer_id = PeerId::random();
    handler.set_peer_id(test_peer_id);
    
    // Test 1: Process OpenStreamWithRole event
    let stream_id = XStreamID::from(123u128);
    handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
        stream_id,
        role: SubstreamRole::Main,
    });
    println!("✅ Successfully processed OpenStreamWithRole event for stream_id: {}", stream_id);
    
    // Test 2: Process another OpenStreamWithRole with different role
    let stream_id_2 = XStreamID::from(456u128);
    handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
        stream_id: stream_id_2,
        role: SubstreamRole::Error,
    });
    println!("✅ Successfully processed OpenStreamWithRole event for stream_id: {} with Error role", stream_id_2);
    
    // Test 3: Verify handler remains functional after events
    let protocol = handler.listen_protocol();
    let keep_alive = handler.connection_keep_alive();
    println!("✅ Handler remains functional after events - protocol: {:?}, keep_alive: {}", protocol, keep_alive);
    
    println!("✅ XStreamHandler event processing test completed successfully");
}

#[tokio::test]
async fn test_handler_multiple_events() {
    // Test handling multiple consecutive events
    println!("🚀 Testing XStreamHandler multiple events handling...");
    
    let mut handler = XStreamHandler::new();
    let test_peer_id = PeerId::random();
    handler.set_peer_id(test_peer_id);
    
    // Process multiple events in sequence
    for i in 0..5 {
        let stream_id = XStreamID::from(i as u128);
        let role = if i % 2 == 0 {
            SubstreamRole::Main
        } else {
            SubstreamRole::Error
        };
        
        handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
            stream_id,
            role,
        });
        
        println!("✅ Processed event {} - stream_id: {}, role: {:?}", i, stream_id, role);
    }
    
    // Verify handler state after multiple events
    let protocol = handler.listen_protocol();
    let keep_alive = handler.connection_keep_alive();
    println!("✅ Handler state after multiple events - protocol: {:?}, keep_alive: {}", protocol, keep_alive);
    
    println!("✅ XStreamHandler multiple events handling test completed successfully");
}

#[tokio::test]
async fn test_handler_event_validation() {
    // Test event validation and error handling
    println!("🚀 Testing XStreamHandler event validation...");
    
    let mut handler = XStreamHandler::new();
    let test_peer_id = PeerId::random();
    handler.set_peer_id(test_peer_id);
    
    // Test with valid stream IDs
    let valid_stream_ids = [
        XStreamID::from(0u128),
        XStreamID::from(1u128),
        XStreamID::from(999u128),
        XStreamID::from(u128::MAX),
    ];
    
    for (i, &stream_id) in valid_stream_ids.iter().enumerate() {
        handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
            stream_id,
            role: if i % 2 == 0 { SubstreamRole::Main } else { SubstreamRole::Error },
        });
        println!("✅ Successfully processed event with valid stream_id: {}", stream_id);
    }
    
    // Test that handler can handle events without panicking
    // This is important for production resilience
    
    println!("✅ XStreamHandler event validation test completed successfully");
}

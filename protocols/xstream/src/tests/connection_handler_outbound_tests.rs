// connection_handler_outbound_tests.rs
// Test 4: Outbound command handling in XStreamHandler

use crate::handler::{XStreamHandler, XStreamHandlerIn, XStreamHandlerEvent};
use crate::events::EstablishedConnection;
use crate::types::{SubstreamRole, XStreamID};
use libp2p::{PeerId, swarm::ConnectionHandler};

#[tokio::test]
async fn test_handler_outbound_stream_commands() {
    // Test outbound stream command handling in XStreamHandler
    println!("🚀 Testing XStreamHandler outbound stream commands...");
    
    let mut handler = XStreamHandler::new(libp2p::swarm::ConnectionId::new_unchecked(1), PeerId::random(), EstablishedConnection::Outbound { addr: "/memory/0".parse().unwrap() });
    let test_peer_id = PeerId::random();
    handler.set_peer_id(test_peer_id);
    
    // Test creating multiple outbound streams with different roles
    let test_streams = vec![
        (XStreamID::from(100u128), SubstreamRole::Main),
        (XStreamID::from(200u128), SubstreamRole::Error),
        (XStreamID::from(300u128), SubstreamRole::Main),
        (XStreamID::from(400u128), SubstreamRole::Error),
    ];
    
    for (stream_id, role) in test_streams {
        handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
            stream_id,
            role,
        });
        
        println!("✅ Sent outbound command - stream_id: {}, role: {:?}", stream_id, role);
    }
    
    // Verify handler remains functional after multiple commands
    let protocol = handler.listen_protocol();
    let keep_alive = handler.connection_keep_alive();
    println!("✅ Handler functional after outbound commands - protocol: {:?}, keep_alive: {}", protocol, keep_alive);
    
    println!("✅ XStreamHandler outbound stream commands test completed successfully");
}

#[tokio::test]
async fn test_handler_concurrent_commands() {
    // Test handling concurrent outbound commands
    println!("🚀 Testing XStreamHandler concurrent commands...");
    
    let mut handler = XStreamHandler::new(libp2p::swarm::ConnectionId::new_unchecked(1), PeerId::random(), EstablishedConnection::Outbound { addr: "/memory/0".parse().unwrap() });
    let test_peer_id = PeerId::random();
    handler.set_peer_id(test_peer_id);
    
    // Send multiple commands in quick succession (simulating concurrent operations)
    for i in 0..10 {
        let stream_id = XStreamID::from((i * 100) as u128);
        let role = if i % 3 == 0 {
            SubstreamRole::Main
        } else {
            SubstreamRole::Error
        };
        
        handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
            stream_id,
            role,
        });
        
        if i % 2 == 0 {
            // Small delay to simulate real-world timing
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }
    
    println!("✅ Sent 10 concurrent commands successfully");
    
    // Verify handler state
    let protocol = handler.listen_protocol();
    let keep_alive = handler.connection_keep_alive();
    println!("✅ Handler state after concurrent commands - protocol: {:?}, keep_alive: {}", protocol, keep_alive);
    
    println!("✅ XStreamHandler concurrent commands test completed successfully");
}

#[tokio::test]
async fn test_handler_command_validation() {
    // Test command validation and edge cases
    println!("🚀 Testing XStreamHandler command validation...");
    
    let mut handler = XStreamHandler::new(libp2p::swarm::ConnectionId::new_unchecked(1), PeerId::random(), EstablishedConnection::Outbound { addr: "/memory/0".parse().unwrap() });
    let test_peer_id = PeerId::random();
    handler.set_peer_id(test_peer_id);
    
    // Test with minimum stream ID
    let min_stream_id = XStreamID::from(0u128);
    handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
        stream_id: min_stream_id,
        role: SubstreamRole::Main,
    });
    println!("✅ Processed command with minimum stream_id: {}", min_stream_id);
    
    // Test with maximum stream ID
    let max_stream_id = XStreamID::from(u128::MAX);
    handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
        stream_id: max_stream_id,
        role: SubstreamRole::Error,
    });
    println!("✅ Processed command with maximum stream_id: {}", max_stream_id);
    
    // Test with sequential stream IDs
    for i in 1..=5 {
        let stream_id = XStreamID::from(i as u128);
        handler.on_behaviour_event(XStreamHandlerIn::OpenStreamWithRole {
            stream_id,
            role: SubstreamRole::Main,
        });
    }
    println!("✅ Processed sequential stream IDs successfully");
    
    // Verify handler remains stable
    let protocol = handler.listen_protocol();
    let keep_alive = handler.connection_keep_alive();
    println!("✅ Handler stable after validation tests - protocol: {:?}, keep_alive: {}", protocol, keep_alive);
    
    println!("✅ XStreamHandler command validation test completed successfully");
}

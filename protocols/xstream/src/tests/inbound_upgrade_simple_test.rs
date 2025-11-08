// Simple test for inbound upgrade functionality
use crate::behaviour::XStreamNetworkBehaviour;
use crate::events::{IncomingConnectionApprovePolicy, InboundUpgradeDecision, XStreamEvent};
use libp2p::{PeerId, swarm::ConnectionId};
use tokio::sync::oneshot;

#[tokio::test]
async fn test_inbound_upgrade_policy_auto_approve() {
    // Test AutoApprove policy
    let behaviour = XStreamNetworkBehaviour::new();
    assert!(matches!(
        behaviour.incoming_approve_policy,
        IncomingConnectionApprovePolicy::AutoApprove
    ));
    println!("✅ AutoApprove policy works correctly");
}

#[tokio::test]
async fn test_inbound_upgrade_policy_approve_via_event() {
    // Test ApproveViaEvent policy
    let behaviour = XStreamNetworkBehaviour::new_with_policy(
        IncomingConnectionApprovePolicy::ApproveViaEvent
    );
    assert!(matches!(
        behaviour.incoming_approve_policy,
        IncomingConnectionApprovePolicy::ApproveViaEvent
    ));
    println!("✅ ApproveViaEvent policy works correctly");
}

#[tokio::test]
async fn test_inbound_upgrade_decision_types() {
    // Test decision types
    let approved = InboundUpgradeDecision::Approved;
    let rejected = InboundUpgradeDecision::Rejected("test reason".to_string());
    
    match approved {
        InboundUpgradeDecision::Approved => println!("✅ Approved decision works"),
        _ => panic!("Expected Approved decision"),
    }
    
    match rejected {
        InboundUpgradeDecision::Rejected(reason) => {
            assert_eq!(reason, "test reason");
            println!("✅ Rejected decision works with reason: {}", reason);
        }
        _ => panic!("Expected Rejected decision"),
    }
}

#[tokio::test]
async fn test_inbound_upgrade_event_creation() {
    // Test that we can create the new event
    let (response_sender, _response_receiver) = oneshot::channel();
    let peer_id = PeerId::random();
    let connection_id = ConnectionId::new_unchecked(1);
    
    let event = XStreamEvent::InboundUpgradeRequest {
        peer_id,
        connection_id,
        response_sender,
    };
    
    match event {
        XStreamEvent::InboundUpgradeRequest { peer_id: p, connection_id: c, .. } => {
            assert_eq!(p, peer_id);
            assert_eq!(c, connection_id);
            println!("✅ InboundUpgradeRequest event creation works");
        }
        _ => panic!("Expected InboundUpgradeRequest event"),
    }
}

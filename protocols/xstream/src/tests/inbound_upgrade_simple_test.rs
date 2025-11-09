// Simple test for inbound upgrade functionality
use crate::behaviour::XStreamNetworkBehaviour;
use crate::events::{IncomingConnectionApprovePolicy, InboundUpgradeDecision, XStreamEvent, StreamOpenDecisionSender};
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
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    let peer_id = PeerId::random();
    let connection_id = ConnectionId::new_unchecked(1);
    
    let event = XStreamEvent::InboundUpgradeRequest {
        peer_id,
        connection_id,
        decision_sender,
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

#[tokio::test]
async fn test_stream_open_decision_sender_api() {
    // Test the new StreamOpenDecisionSender API
    let (response_sender, response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    
    // Test approve
    let result = decision_sender.approve();
    assert!(result.is_ok());
    println!("✅ StreamOpenDecisionSender::approve() works");
    
    // Verify decision
    match response_receiver.await {
        Ok(InboundUpgradeDecision::Approved) => println!("✅ Approved decision received correctly"),
        _ => panic!("Expected Approved decision"),
    }
}

#[tokio::test]
async fn test_stream_open_decision_sender_reject_api() {
    // Test reject functionality
    let (response_sender, response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    
    // Test reject with reason
    let reason = "test rejection reason".to_string();
    let result = decision_sender.reject(reason.clone());
    assert!(result.is_ok());
    println!("✅ StreamOpenDecisionSender::reject() works");
    
    // Verify decision
    match response_receiver.await {
        Ok(InboundUpgradeDecision::Rejected(received_reason)) => {
            assert_eq!(received_reason, reason);
            println!("✅ Rejected decision with reason works");
        }
        _ => panic!("Expected Rejected decision"),
    }
}

use crate::behaviour::XStreamNetworkBehaviour;
use crate::events::{IncomingConnectionApprovePolicy, InboundUpgradeDecision, XStreamEvent, StreamOpenDecisionSender};
use libp2p::{PeerId, swarm::{ConnectionId, SwarmEvent}};
use tokio::sync::oneshot;

#[tokio::test]
async fn test_inbound_upgrade_auto_approve() {
    // Test that AutoApprove policy automatically approves incoming upgrades
    let behaviour = XStreamNetworkBehaviour::new();
    // This should compile and work without generating events
    assert!(matches!(
        behaviour.incoming_approve_policy,
        IncomingConnectionApprovePolicy::AutoApprove
    ));
}

#[tokio::test]
async fn test_inbound_upgrade_approve_via_event() {
    // Test that ApproveViaEvent policy creates the new constructor
    let behaviour = XStreamNetworkBehaviour::new_with_policy(
        IncomingConnectionApprovePolicy::ApproveViaEvent
    );
    assert!(matches!(
        behaviour.incoming_approve_policy,
        IncomingConnectionApprovePolicy::ApproveViaEvent
    ));
}

#[tokio::test]
async fn test_inbound_upgrade_decision_types() {
    // Test that the decision types work correctly
    let decision_approved = InboundUpgradeDecision::Approved;
    let decision_rejected = InboundUpgradeDecision::Rejected("test reason".to_string());
    
    match decision_approved {
        InboundUpgradeDecision::Approved => assert!(true),
        _ => panic!("Expected Approved decision"),
    }
    
    match decision_rejected {
        InboundUpgradeDecision::Rejected(reason) => assert_eq!(reason, "test reason"),
        _ => panic!("Expected Rejected decision"),
    }
}

#[tokio::test]
async fn test_inbound_upgrade_request_event() {
    // Test that the IncomingStreamRequest event can be created
    let (response_sender, _response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    let peer_id = PeerId::random();
    let connection_id = ConnectionId::new_unchecked(1);
    
    let event = XStreamEvent::IncomingStreamRequest {
        peer_id,
        connection_id,
        decision_sender,
    };
    
    match event {
        XStreamEvent::IncomingStreamRequest { peer_id: p, connection_id: c, .. } => {
            assert_eq!(p, peer_id);
            assert_eq!(c, connection_id);
        }
        _ => panic!("Expected IncomingStreamRequest event"),
    }
}

#[tokio::test]
async fn test_stream_open_decision_sender() {
    // Test that StreamOpenDecisionSender works correctly
    let (response_sender, response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    
    // Test approve
    let result = decision_sender.approve();
    assert!(result.is_ok());
    
    // Verify the decision was sent
    match response_receiver.await {
        Ok(decision) => {
            match decision {
                InboundUpgradeDecision::Approved => assert!(true),
                _ => panic!("Expected Approved decision"),
            }
        }
        Err(_) => panic!("Failed to receive decision"),
    }
}

#[tokio::test]
async fn test_stream_open_decision_sender_reject() {
    // Test that StreamOpenDecisionSender reject works correctly
    let (response_sender, response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    
    // Test reject
    let result = decision_sender.reject("test reason".to_string());
    assert!(result.is_ok());
    
    // Verify the decision was sent
    match response_receiver.await {
        Ok(decision) => {
            match decision {
                InboundUpgradeDecision::Rejected(reason) => assert_eq!(reason, "test reason"),
                _ => panic!("Expected Rejected decision"),
            }
        }
        Err(_) => panic!("Failed to receive decision"),
    }
}

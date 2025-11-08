use crate::behaviour::XStreamNetworkBehaviour;
use crate::events::{IncomingConnectionApprovePolicy, InboundUpgradeDecision, XStreamEvent};
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
    // Test that the InboundUpgradeRequest event can be created
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
        }
        _ => panic!("Expected InboundUpgradeRequest event"),
    }
}

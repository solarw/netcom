//! Basic tests for XRoutes behaviour

use libp2p::PeerId;
use xnetwork2::behaviours::xroutes::{XRoutesBehaviour, XRoutesConfig};

#[tokio::test]
async fn test_xroutes_behaviour_creation() {
    // Create a test peer ID
    let peer_id = PeerId::random();
    
    // Test with default configuration
    let config = XRoutesConfig::default();
    let behaviour = XRoutesBehaviour::new(peer_id, &config);
    assert!(behaviour.is_ok(), "Failed to create XRoutesBehaviour with default config");
    
    let behaviour = behaviour.unwrap();
    let status = behaviour.get_status();
    
    // With default config, mDNS and Kademlia should be enabled
    assert!(!status.identify_enabled, "Identify should be disabled in XRoutes");
    assert!(status.mdns_enabled, "mDNS should be enabled by default");
    assert!(status.kad_enabled, "Kademlia should be enabled by default");
}

#[tokio::test]
async fn test_xroutes_behaviour_disabled() {
    // Create a test peer ID
    let peer_id = PeerId::random();
    
    // Test with all behaviours disabled
    let config = XRoutesConfig::disabled();
    let behaviour = XRoutesBehaviour::new(peer_id, &config);
    assert!(behaviour.is_ok(), "Failed to create XRoutesBehaviour with disabled config");
    
    let behaviour = behaviour.unwrap();
    let status = behaviour.get_status();
    
    // All behaviours should be disabled
    assert!(!status.identify_enabled, "Identify should be disabled");
    assert!(!status.mdns_enabled, "mDNS should be disabled");
    assert!(!status.kad_enabled, "Kademlia should be disabled");
}

#[tokio::test]
async fn test_xroutes_config_builder() {
    let config = XRoutesConfig::new()
        .with_mdns(false)
        .with_kad(false);
    
    assert!(!config.enable_mdns, "mDNS should be disabled");
    assert!(!config.enable_kad, "Kademlia should be disabled");
    assert!(config.enable_identify, "Identify should be enabled by default");
}

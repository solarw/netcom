//! Basic tests for XRoutes behaviour

use libp2p::{identity::Keypair, PeerId};
use xnetwork2::behaviours::xroutes::{XRoutesBehaviour, XRoutesConfig};

#[tokio::test]
async fn test_xroutes_behaviour_creation() {
    // Create a test keypair and get public key
    let keypair = Keypair::generate_ed25519();
    let public_key = keypair.public();
    let peer_id = public_key.to_peer_id();
    
    // Test with default configuration
    let config = XRoutesConfig::default();
    
    // Pass None for relay_client in tests
    let behaviour = XRoutesBehaviour::new(public_key, &config, None);
    assert!(behaviour.is_ok(), "Failed to create XRoutesBehaviour with default config");
    
    let behaviour = behaviour.unwrap();
    let status = behaviour.get_status();
    
    // With default config, identify, mDNS and Kademlia should be enabled
    assert!(status.identify_enabled, "Identify should be enabled by default");
    assert!(status.mdns_enabled, "mDNS should be enabled by default");
    assert!(status.kad_enabled, "Kademlia should be enabled by default");
    assert!(!status.relay_client_enabled, "Relay client should be disabled in tests");
}

#[tokio::test]
async fn test_xroutes_behaviour_disabled() {
    // Create a test keypair and get public key
    let keypair = Keypair::generate_ed25519();
    let public_key = keypair.public();
    let peer_id = public_key.to_peer_id();
    
    // Test with all behaviours disabled
    let config = XRoutesConfig::disabled();
    
    // Pass None for relay_client in tests
    let behaviour = XRoutesBehaviour::new(public_key, &config, None);
    assert!(behaviour.is_ok(), "Failed to create XRoutesBehaviour with disabled config");
    
    let behaviour = behaviour.unwrap();
    let status = behaviour.get_status();
    
    // All behaviours should be disabled
    assert!(!status.identify_enabled, "Identify should be disabled");
    assert!(!status.mdns_enabled, "mDNS should be disabled");
    assert!(!status.kad_enabled, "Kademlia should be disabled");
    assert!(!status.relay_client_enabled, "Relay client should be disabled in tests");
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

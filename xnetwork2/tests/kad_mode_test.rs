//! Test for Kademlia mode switching functionality

use std::time::Duration;
use tokio::time::timeout;

use xnetwork2::{
    commander::Commander,
    xroutes::{
        types::{KadMode, XRoutesConfig},
    },
};

/// Test setting Kademlia mode to client
#[tokio::test]
async fn test_set_kad_mode_client() {
    let config = XRoutesConfig {
        enable_kad: true,
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Set mode to client
    let result = commander.set_kad_mode(KadMode::Client).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to client: {:?}", result);
    
    // Verify mode is client
    let mode = commander.get_kad_mode().await;
    assert!(mode.is_ok(), "Failed to get Kademlia mode: {:?}", mode);
    assert_eq!(mode.unwrap(), KadMode::Client, "Kademlia mode should be Client");
}

/// Test setting Kademlia mode to server
#[tokio::test]
async fn test_set_kad_mode_server() {
    let config = XRoutesConfig {
        enable_kad: true,
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Set mode to server
    let result = commander.set_kad_mode(KadMode::Server).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to server: {:?}", result);
    
    // Verify mode is server
    let mode = commander.get_kad_mode().await;
    assert!(mode.is_ok(), "Failed to get Kademlia mode: {:?}", mode);
    assert_eq!(mode.unwrap(), KadMode::Server, "Kademlia mode should be Server");
}

/// Test setting Kademlia mode to auto
#[tokio::test]
async fn test_set_kad_mode_auto() {
    let config = XRoutesConfig {
        enable_kad: true,
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Set mode to auto
    let result = commander.set_kad_mode(KadMode::Auto).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to auto: {:?}", result);
    
    // Note: Auto mode is represented as None in the underlying Kademlia behaviour
    // The get_kad_mode will return either Client or Server based on current state
    let mode = commander.get_kad_mode().await;
    assert!(mode.is_ok(), "Failed to get Kademlia mode: {:?}", mode);
    // Auto mode can result in either Client or Server depending on network conditions
    let mode_value = mode.unwrap();
    assert!(
        mode_value == KadMode::Client || mode_value == KadMode::Server,
        "Kademlia mode should be either Client or Server in auto mode, got: {:?}",
        mode_value
    );
}

/// Test switching between different modes
#[tokio::test]
async fn test_kad_mode_switching() {
    let config = XRoutesConfig {
        enable_kad: true,
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Test Client -> Server
    let result = commander.set_kad_mode(KadMode::Client).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to client: {:?}", result);
    
    let mode = commander.get_kad_mode().await.unwrap();
    assert_eq!(mode, KadMode::Client, "Should be in Client mode");
    
    let result = commander.set_kad_mode(KadMode::Server).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to server: {:?}", result);
    
    let mode = commander.get_kad_mode().await.unwrap();
    assert_eq!(mode, KadMode::Server, "Should be in Server mode");
    
    // Test Server -> Auto
    let result = commander.set_kad_mode(KadMode::Auto).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to auto: {:?}", result);
    
    let mode = commander.get_kad_mode().await.unwrap();
    assert!(
        mode == KadMode::Client || mode == KadMode::Server,
        "Should be in either Client or Server mode in auto"
    );
}

/// Test setting mode when Kademlia is disabled
#[tokio::test]
async fn test_set_kad_mode_when_disabled() {
    let config = XRoutesConfig {
        enable_kad: false, // Kademlia disabled
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Try to set mode when Kademlia is disabled
    let result = commander.set_kad_mode(KadMode::Client).await;
    assert!(result.is_err(), "Should fail to set mode when Kademlia is disabled");
    
    // Try to get mode when Kademlia is disabled
    let result = commander.get_kad_mode().await;
    assert!(result.is_err(), "Should fail to get mode when Kademlia is disabled");
}

/// Test integration with status command
#[tokio::test]
async fn test_kad_mode_in_status() {
    let config = XRoutesConfig {
        enable_kad: true,
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Set mode to server
    let result = commander.set_kad_mode(KadMode::Server).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to server");
    
    // Check status includes Kademlia mode
    let status = commander.get_status().await.unwrap();
    assert!(status.kad_enabled, "Kademlia should be enabled in status");
    assert_eq!(status.kad_mode, Some(KadMode::Server), "Kademlia mode should be Server in status");
    
    // Switch to client and check again
    let result = commander.set_kad_mode(KadMode::Client).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to client");
    
    let status = commander.get_status().await.unwrap();
    assert_eq!(status.kad_mode, Some(KadMode::Client), "Kademlia mode should be Client in status");
}

/// Test timeout handling for mode operations
#[tokio::test]
async fn test_kad_mode_timeout() {
    let config = XRoutesConfig {
        enable_kad: true,
        ..Default::default()
    };
    
    let xroutes = XRoutes::new(config).await.unwrap();
    let commander = Commander::new(xroutes).await.unwrap();
    
    // Set mode with timeout - should complete quickly
    let result = timeout(
        Duration::from_secs(5),
        commander.set_kad_mode(KadMode::Server)
    ).await;
    
    assert!(result.is_ok(), "Set mode operation timed out");
    let inner_result = result.unwrap();
    assert!(inner_result.is_ok(), "Set mode operation failed: {:?}", inner_result);
    
    // Get mode with timeout - should complete quickly
    let result = timeout(
        Duration::from_secs(5),
        commander.get_kad_mode()
    ).await;
    
    assert!(result.is_ok(), "Get mode operation timed out");
    let inner_result = result.unwrap();
    assert!(inner_result.is_ok(), "Get mode operation failed: {:?}", inner_result);
    assert_eq!(inner_result.unwrap(), KadMode::Server, "Mode should be Server");
}

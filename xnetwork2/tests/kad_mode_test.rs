//! Test for Kademlia mode switching functionality

use std::time::Duration;
use tokio::time::timeout;

use xnetwork2::{
    node_builder,
    behaviours::xroutes::types::KadMode,
};

/// Test setting Kademlia mode to client
#[tokio::test]
async fn test_set_kad_mode_client() {
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Set mode to client
    let result = node.set_kad_mode(KadMode::Client).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to client: {:?}", result);

    // Verify mode is client
    let mode = node.get_kad_mode().await;
    assert!(mode.is_ok(), "Failed to get Kademlia mode: {:?}", mode);
    assert_eq!(mode.unwrap(), KadMode::Client, "Kademlia mode should be Client");

    node.force_shutdown().await.unwrap();
}

/// Test setting Kademlia mode to server
#[tokio::test]
async fn test_set_kad_mode_server() {
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Set mode to server
    let result = node.set_kad_mode(KadMode::Server).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to server: {:?}", result);

    // Verify mode is server
    let mode = node.get_kad_mode().await;
    assert!(mode.is_ok(), "Failed to get Kademlia mode: {:?}", mode);
    assert_eq!(mode.unwrap(), KadMode::Server, "Kademlia mode should be Server");

    node.force_shutdown().await.unwrap();
}

/// Test setting Kademlia mode to auto
#[tokio::test]
async fn test_set_kad_mode_auto() {
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Set mode to auto
    let result = node.set_kad_mode(KadMode::Auto).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to auto: {:?}", result);

    // Note: Auto mode is represented as None in the underlying Kademlia behaviour
    // The get_kad_mode will return either Client or Server based on current state
    let mode = node.get_kad_mode().await;
    assert!(mode.is_ok(), "Failed to get Kademlia mode: {:?}", mode);
    // Auto mode can result in either Client or Server depending on network conditions
    let mode_value = mode.unwrap();
    assert!(
        mode_value == KadMode::Client || mode_value == KadMode::Server,
        "Kademlia mode should be either Client or Server in auto mode, got: {:?}",
        mode_value
    );

    node.force_shutdown().await.unwrap();
}

/// Test switching between different modes
#[tokio::test]
async fn test_kad_mode_switching() {
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Test Client -> Server
    let result = node.set_kad_mode(KadMode::Client).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to client: {:?}", result);

    let mode = node.get_kad_mode().await.unwrap();
    assert_eq!(mode, KadMode::Client, "Should be in Client mode");

    let result = node.set_kad_mode(KadMode::Server).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to server: {:?}", result);

    let mode = node.get_kad_mode().await.unwrap();
    assert_eq!(mode, KadMode::Server, "Should be in Server mode");

    // Test Server -> Auto
    let result = node.set_kad_mode(KadMode::Auto).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to auto: {:?}", result);

    let mode = node.get_kad_mode().await.unwrap();
    assert!(
        mode == KadMode::Client || mode == KadMode::Server,
        "Should be in either Client or Server mode in auto"
    );

    node.force_shutdown().await.unwrap();
}

/// Test setting mode when Kademlia is disabled
#[tokio::test]
async fn test_set_kad_mode_when_disabled() {
    let mut node = node_builder::builder()
        // Kademlia disabled by default
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Try to set mode when Kademlia is disabled
    let result = node.set_kad_mode(KadMode::Client).await;
    assert!(result.is_err(), "Should fail to set mode when Kademlia is disabled");

    // Try to get mode when Kademlia is disabled
    let result = node.get_kad_mode().await;
    assert!(result.is_err(), "Should fail to get mode when Kademlia is disabled");

    node.force_shutdown().await.unwrap();
}

/// Test integration with status command
#[tokio::test]
async fn test_kad_mode_in_status() {
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Set mode to server
    let result = node.set_kad_mode(KadMode::Server).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to server");

    // Check status includes Kademlia mode
    let status = node.get_xroutes_status().await.unwrap();
    assert!(status.kad_enabled, "Kademlia should be enabled in status");
    assert_eq!(status.kad_mode, Some(KadMode::Server), "Kademlia mode should be Server in status");

    // Switch to client and check again
    let result = node.set_kad_mode(KadMode::Client).await;
    assert!(result.is_ok(), "Failed to set Kademlia mode to client");

    let status = node.get_xroutes_status().await.unwrap();
    assert_eq!(status.kad_mode, Some(KadMode::Client), "Kademlia mode should be Client in status");

    node.force_shutdown().await.unwrap();
}

/// Test timeout handling for mode operations
#[tokio::test]
async fn test_kad_mode_timeout() {
    let mut node = node_builder::builder()
        .with_kademlia()
        .build()
        .await
        .unwrap();
    node.start().await.unwrap();

    // Set mode with timeout - should complete quickly
    let result = timeout(
        Duration::from_secs(5),
        node.set_kad_mode(KadMode::Server)
    ).await;

    assert!(result.is_ok(), "Set mode operation timed out");
    let inner_result = result.unwrap();
    assert!(inner_result.is_ok(), "Set mode operation failed: {:?}", inner_result);

    // Get mode with timeout - should complete quickly
    let result = timeout(
        Duration::from_secs(5),
        node.get_kad_mode()
    ).await;

    assert!(result.is_ok(), "Get mode operation timed out");
    let inner_result = result.unwrap();
    assert!(inner_result.is_ok(), "Get mode operation failed: {:?}", inner_result);
    assert_eq!(inner_result.unwrap(), KadMode::Server, "Mode should be Server");

    node.force_shutdown().await.unwrap();
}

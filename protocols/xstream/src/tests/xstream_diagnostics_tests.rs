// xstream_diagnostics_tests.rs
// Diagnostic tests to identify issues in the XStream implementation

use crate::types::{XStreamDirection, XStreamID, XStreamState};
use crate::xstream::XStream;
use crate::xstream_error::{ErrorOnRead, ReadError, XStreamError};
use futures::{AsyncReadExt, AsyncWriteExt};
use libp2p::PeerId;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

// Import helper function to create test pairs
use super::xstream_tests::create_xstream_test_pair;

// Helper function with shorter timeout for diagnostic tests
async fn with_timeout<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    match timeout(Duration::from_secs(5), future).await {
        Ok(result) => result,
        Err(_) => panic!("Diagnostic test operation timed out after 5 seconds"),
    }
}

// Diagnostic Test 1: Check basic stream creation and properties
#[tokio::test]
async fn diagnostic_stream_creation() {
    let (test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Stream Creation Diagnostics ===");
    println!("Client stream ID: {:?}", test_pair.client_stream.id);
    println!("Server stream ID: {:?}", test_pair.server_stream.id);
    println!("Client peer ID: {}", test_pair.client_peer_id);
    println!("Server peer ID: {}", test_pair.server_peer_id);
    println!("Client direction: {:?}", test_pair.client_stream.direction);
    println!("Server direction: {:?}", test_pair.server_stream.direction);
    println!("Client state: {:?}", test_pair.client_stream.state());
    println!("Server state: {:?}", test_pair.server_stream.state());

    // Basic assertions
    assert_eq!(test_pair.client_stream.direction, XStreamDirection::Outbound);
    assert_eq!(test_pair.server_stream.direction, XStreamDirection::Inbound);
    assert_eq!(test_pair.client_stream.state(), XStreamState::Open);
    assert_eq!(test_pair.server_stream.state(), XStreamState::Open);

    println!("✓ Stream creation diagnostics passed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 2: Check error stream component states
#[tokio::test]
async fn diagnostic_error_stream_components() {
    let (test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Error Stream Components Diagnostics ===");
    
    // Check error data availability
    let client_has_error = test_pair.client_stream.has_error_data().await;
    let client_has_pending = test_pair.client_stream.has_pending_error().await;
    let client_cached_error = test_pair.client_stream.get_cached_error().await;
    
    println!("Client has error data: {}", client_has_error);
    println!("Client has pending error: {}", client_has_pending);
    println!("Client cached error: {:?}", client_cached_error);

    // Initially should have no errors
    assert!(!client_has_error);
    assert!(!client_has_pending);
    assert_eq!(client_cached_error, None);

    println!("✓ Error stream components diagnostics passed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 3: Test simple data flow before testing errors
#[tokio::test]
async fn diagnostic_simple_data_flow() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Simple Data Flow Diagnostics ===");
    
    // Test basic write/read
    let test_data = b"Hello, diagnostic test!".to_vec();
    println!("Sending data: {:?}", String::from_utf8_lossy(&test_data));
    
    // Client writes data
    let write_result = with_timeout(test_pair.client_stream.write_all(test_data.clone())).await;
    println!("Client write result: {:?}", write_result);
    assert!(write_result.is_ok());

    let flush_result = with_timeout(test_pair.client_stream.flush()).await;
    println!("Client flush result: {:?}", flush_result);
    assert!(flush_result.is_ok());

    // Server reads data
    let read_result = with_timeout(test_pair.server_stream.read_exact(test_data.len())).await;
    println!("Server read result: {:?}", read_result);
    assert!(read_result.is_ok());
    
    let received_data = read_result.unwrap();
    println!("Received data: {:?}", String::from_utf8_lossy(&received_data));
    assert_eq!(received_data, test_data);

    println!("✓ Simple data flow diagnostics passed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 4: Test error write mechanics step by step
#[tokio::test]
async fn diagnostic_error_write_mechanics() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Error Write Mechanics Diagnostics ===");
    
    // Check initial server state
    println!("Initial server state: {:?}", test_pair.server_stream.state());
    println!("Server is write local closed: {}", test_pair.server_stream.is_write_local_closed());
    
    // Try to write error
    let error_message = b"Diagnostic error message".to_vec();
    println!("Attempting to write error: {:?}", String::from_utf8_lossy(&error_message));
    
    let error_write_result = with_timeout(test_pair.server_stream.error_write(error_message.clone())).await;
    println!("Error write result: {:?}", error_write_result);
    
    match error_write_result {
        Ok(_) => {
            println!("✓ Error write succeeded");
            
            // Check server state after error write
            println!("Server state after error write: {:?}", test_pair.server_stream.state());
            println!("Server is write local closed: {}", test_pair.server_stream.is_write_local_closed());
            println!("Server is closed: {}", test_pair.server_stream.is_closed());
        }
        Err(e) => {
            println!("✗ Error write failed: {:?}", e);
            println!("Error kind: {:?}", e.kind());
        }
    }

    println!("✓ Error write mechanics diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 5: Test error read mechanics step by step
#[tokio::test]
async fn diagnostic_error_read_mechanics() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Error Read Mechanics Diagnostics ===");
    
    // First write an error from server
    let error_message = b"Diagnostic read test error".to_vec();
    let write_result = with_timeout(test_pair.server_stream.error_write(error_message.clone())).await;
    
    if write_result.is_err() {
        println!("✗ Cannot test error read - error write failed: {:?}", write_result);
        with_timeout(shutdown_manager.shutdown()).await;
        return;
    }
    
    println!("✓ Error written successfully");
    
    // Wait a bit for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check client error state
    println!("Client has error data: {}", test_pair.client_stream.has_error_data().await);
    println!("Client has pending error: {}", test_pair.client_stream.has_pending_error().await);
    
    let cached_error = test_pair.client_stream.get_cached_error().await;
    println!("Client cached error: {:?}", cached_error.as_ref().map(|e| String::from_utf8_lossy(e)));
    
    // Try to read error
    println!("Attempting to read error...");
    let error_read_result = with_timeout(test_pair.client_stream.error_read()).await;
    
    match error_read_result {
        Ok(received_error) => {
            println!("✓ Error read succeeded");
            println!("Received error: {:?}", String::from_utf8_lossy(&received_error));
            assert_eq!(received_error, error_message);
        }
        Err(e) => {
            println!("✗ Error read failed: {:?}", e);
            println!("Error kind: {:?}", e.kind());
        }
    }

    println!("✓ Error read mechanics diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 6: Test data read after error is sent
#[tokio::test]
async fn diagnostic_data_read_after_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Data Read After Error Diagnostics ===");
    
    // Send some data first
    let initial_data = b"Data before error".to_vec();
    with_timeout(test_pair.server_stream.write_all(initial_data.clone())).await
        .expect("Failed to send initial data");
    with_timeout(test_pair.server_stream.flush()).await
        .expect("Failed to flush initial data");
    
    // Client reads the initial data
    let received_initial = with_timeout(test_pair.client_stream.read_exact(initial_data.len())).await
        .expect("Failed to read initial data");
    assert_eq!(received_initial, initial_data);
    println!("✓ Initial data received correctly");
    
    // Server sends error
    let error_message = b"Error after data".to_vec();
    let error_write_result = with_timeout(test_pair.server_stream.error_write(error_message.clone())).await;
    
    if error_write_result.is_err() {
        println!("✗ Error write failed: {:?}", error_write_result);
        with_timeout(shutdown_manager.shutdown()).await;
        return;
    }
    
    // Now try to read more data - should get error
    println!("Attempting to read data after error was sent...");
    let data_read_result = with_timeout(test_pair.client_stream.read()).await;
    
    match data_read_result {
        Ok(data) => {
            println!("Data read returned: {} bytes", data.len());
            if data.is_empty() {
                println!("✓ Got EOF as expected");
            } else {
                println!("Got unexpected data: {:?}", String::from_utf8_lossy(&data));
            }
        }
        Err(error_on_read) => {
            println!("Data read returned error (this might be expected)");
            if error_on_read.is_xstream_error() {
                println!("✓ Got XStream error as expected");
                if let Some(xs_error) = error_on_read.as_xstream_error() {
                    println!("Error message: {:?}", String::from_utf8_lossy(xs_error.data()));
                }
            } else if error_on_read.is_io_error() {
                println!("Got IO error: {:?}", error_on_read.as_io_error());
            }
        }
    }
    
    // Also try to read error explicitly
    println!("Attempting to read error explicitly...");
    let error_read_result = with_timeout(test_pair.client_stream.error_read()).await;
    
    match error_read_result {
        Ok(received_error) => {
            println!("✓ Error read succeeded: {:?}", String::from_utf8_lossy(&received_error));
            assert_eq!(received_error, error_message);
        }
        Err(e) => {
            println!("Error read failed: {:?}", e);
        }
    }

    println!("✓ Data read after error diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 7: Check background task behavior
#[tokio::test]
async fn diagnostic_background_tasks() {
    let (test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Background Tasks Diagnostics ===");
    
    // Check if error reader tasks are running for outbound streams
    println!("Client stream direction: {:?}", test_pair.client_stream.direction);
    println!("Server stream direction: {:?}", test_pair.server_stream.direction);
    
    // Only outbound streams should have error reader tasks
    if test_pair.client_stream.direction == XStreamDirection::Outbound {
        println!("✓ Client stream is outbound - should have error reader task");
    }
    
    if test_pair.server_stream.direction == XStreamDirection::Inbound {
        println!("✓ Server stream is inbound - should NOT have error reader task");
    }
    
    // Wait a bit to let any background tasks initialize
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✓ Background tasks diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 8: Test stream splitting and reuniting issues
#[tokio::test]
async fn diagnostic_stream_splitting() {
    let (test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Stream Splitting Diagnostics ===");
    
    // This test is to understand how the current implementation handles stream splitting
    // We can't directly test the internal split/reunite logic, but we can observe its effects
    
    println!("Client stream has main read/write components");
    println!("Client stream has error read/write components");
    println!("Server stream has main read/write components");
    println!("Server stream has error read/write components");
    
    // Check if streams are properly initialized
    println!("Client stream state: {:?}", test_pair.client_stream.state());
    println!("Server stream state: {:?}", test_pair.server_stream.state());
    
    println!("✓ Stream splitting diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 9: Test error permissions in detail
#[tokio::test]
async fn diagnostic_error_permissions() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Error Permissions Diagnostics ===");
    
    // Test client trying to write error (should fail)
    println!("Testing client error write permission (should fail)...");
    let client_error = b"Client error attempt".to_vec();
    let client_write_result = with_timeout(test_pair.client_stream.error_write(client_error)).await;
    
    match client_write_result {
        Ok(_) => println!("✗ UNEXPECTED: Client was able to write error!"),
        Err(e) => {
            println!("✓ EXPECTED: Client error write failed with: {:?}", e);
            assert_eq!(e.kind(), ErrorKind::PermissionDenied);
        }
    }
    
    // Test server trying to read error (should fail)
    println!("Testing server error read permission (should fail)...");
    let server_read_result = with_timeout(test_pair.server_stream.error_read()).await;
    
    match server_read_result {
        Ok(_) => println!("✗ UNEXPECTED: Server was able to read error!"),
        Err(e) => {
            println!("✓ EXPECTED: Server error read failed with: {:?}", e);
            assert_eq!(e.kind(), ErrorKind::PermissionDenied);
        }
    }
    
    // Test valid operations
    println!("Testing valid server error write...");
    let server_error = b"Valid server error".to_vec();
    let server_write_result = with_timeout(test_pair.server_stream.error_write(server_error.clone())).await;
    
    match server_write_result {
        Ok(_) => {
            println!("✓ Server error write succeeded");
            
            println!("Testing valid client error read...");
            let client_read_result = with_timeout(test_pair.client_stream.error_read()).await;
            
            match client_read_result {
                Ok(received_error) => {
                    println!("✓ Client error read succeeded");
                    assert_eq!(received_error, server_error);
                }
                Err(e) => println!("✗ Client error read failed: {:?}", e),
            }
        }
        Err(e) => println!("✗ Server error write failed: {:?}", e),
    }

    println!("✓ Error permissions diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 10: Test concurrent operations and timing
#[tokio::test]
async fn diagnostic_concurrent_timing() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Concurrent Timing Diagnostics ===");
    
    // Start multiple operations concurrently to see how they interact
    
    // Task 1: Client sends data continuously
    let client_stream = test_pair.client_stream.clone();
    let data_sender = tokio::spawn(async move {
        for i in 0..5 {
            let data = format!("Message {}", i).into_bytes();
            match client_stream.write_all(data.clone()).await {
                Ok(_) => {
                    println!("Sent message {}", i);
                    if let Err(e) = client_stream.flush().await {
                        println!("Flush failed for message {}: {:?}", i, e);
                        break;
                    }
                }
                Err(e) => {
                    println!("Write failed for message {}: {:?}", i, e);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    
    // Task 2: Server reads data for a while then sends error
    let server_stream = test_pair.server_stream.clone();
    let error_sender = tokio::spawn(async move {
        let mut received_count = 0;
        
        // Read a few messages
        for _ in 0..3 {
            match timeout(Duration::from_millis(200), server_stream.read()).await {
                Ok(Ok(data)) if !data.is_empty() => {
                    received_count += 1;
                    println!("Server received message {}: {:?}", received_count, String::from_utf8_lossy(&data));
                }
                Ok(Ok(_)) => {
                    println!("Server received empty data");
                    break;
                }
                Ok(Err(e)) => {
                    println!("Server read error: {:?}", e);
                    break;
                }
                Err(_) => {
                    println!("Server read timeout");
                    break;
                }
            }
        }
        
        // Send error after receiving some messages
        println!("Server sending error after receiving {} messages", received_count);
        let error_msg = format!("Error after {} messages", received_count).into_bytes();
        match server_stream.error_write(error_msg).await {
            Ok(_) => println!("✓ Server error sent successfully"),
            Err(e) => println!("✗ Server error send failed: {:?}", e),
        }
    });
    
    // Wait for both tasks to complete
    let (data_result, error_result) = tokio::join!(data_sender, error_sender);
    
    match data_result {
        Ok(_) => println!("✓ Data sender task completed"),
        Err(e) => println!("Data sender task failed: {:?}", e),
    }
    
    match error_result {
        Ok(_) => println!("✓ Error sender task completed"),
        Err(e) => println!("Error sender task failed: {:?}", e),
    }
    
    // Try to read the error from client
    println!("Client attempting to read error...");
    match timeout(Duration::from_millis(500), test_pair.client_stream.error_read()).await {
        Ok(Ok(error)) => println!("✓ Client received error: {:?}", String::from_utf8_lossy(&error)),
        Ok(Err(e)) => println!("Client error read failed: {:?}", e),
        Err(_) => println!("Client error read timed out"),
    }

    println!("✓ Concurrent timing diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Diagnostic Test 11: Memory and resource usage
#[tokio::test]
async fn diagnostic_resource_usage() {
    println!("=== Resource Usage Diagnostics ===");
    
    // Create multiple stream pairs to check for resource leaks
    let mut pairs = Vec::new();
    let mut shutdown_managers = Vec::new();
    
    for i in 0..3 {
        println!("Creating stream pair {}", i);
        let (pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;
        pairs.push(pair);
        shutdown_managers.push(shutdown_manager);
    }
    
    println!("✓ Created {} stream pairs", pairs.len());
    
    // Use each pair briefly
    for (i, mut pair) in pairs.into_iter().enumerate() {
        let test_data = format!("Test data from pair {}", i).into_bytes();
        
        if let Ok(_) = with_timeout(pair.client_stream.write_all(test_data.clone())).await {
            if let Ok(_) = with_timeout(pair.client_stream.flush()).await {
                if let Ok(received) = with_timeout(pair.server_stream.read_exact(test_data.len())).await {
                    if received == test_data {
                        println!("✓ Pair {} worked correctly", i);
                    }
                }
            }
        }
    }
    
    // Clean up all pairs
    for (i, shutdown_manager) in shutdown_managers.into_iter().enumerate() {
        println!("Shutting down pair {}", i);
        with_timeout(shutdown_manager.shutdown()).await;
    }
    
    println!("✓ Resource usage diagnostics completed");
}

// Diagnostic Test 12: Test edge case scenarios
#[tokio::test]
async fn diagnostic_edge_cases() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Edge Cases Diagnostics ===");
    
    // Edge Case 1: Try to read before any data is sent
    println!("Testing read before any data...");
    match timeout(Duration::from_millis(100), test_pair.client_stream.read()).await {
        Ok(Ok(data)) => println!("Read returned {} bytes", data.len()),
        Ok(Err(e)) => println!("Read returned error: {:?}", e),
        Err(_) => println!("✓ Read timed out as expected (no data available)"),
    }
    
    // Edge Case 2: Try to read error before any error is sent
    println!("Testing error read before error is sent...");
    match timeout(Duration::from_millis(100), test_pair.client_stream.error_read()).await {
        Ok(Ok(error)) => println!("Unexpected: got error data: {:?}", String::from_utf8_lossy(&error)),
        Ok(Err(e)) => println!("Error read failed: {:?}", e),
        Err(_) => println!("✓ Error read timed out as expected (no error available)"),
    }
    
    // Edge Case 3: Try operations after closing
    println!("Testing operations after close...");
    with_timeout(test_pair.client_stream.close()).await.expect("Failed to close client stream");
    
    let write_after_close = with_timeout(test_pair.client_stream.write_all(b"after close".to_vec())).await;
    match write_after_close {
        Ok(_) => println!("✗ UNEXPECTED: Write succeeded after close"),
        Err(e) => println!("✓ EXPECTED: Write failed after close: {:?}", e),
    }
    
    println!("✓ Edge cases diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}

// Summary diagnostic test that runs a complete error scenario
#[tokio::test]
async fn diagnostic_complete_error_scenario() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    println!("=== Complete Error Scenario Diagnostics ===");
    
    // Step 1: Normal operation
    println!("Step 1: Testing normal operation...");
    let request = b"Normal request".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone())).await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush()).await
        .expect("Failed to flush request");
    
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len())).await
        .expect("Failed to read request");
    assert_eq!(received_request, request);
    println!("✓ Normal operation successful");
    
    // Step 2: Partial response
    println!("Step 2: Server sending partial response...");
    let partial_response = b"Partial response data".to_vec();
    with_timeout(test_pair.server_stream.write_all(partial_response.clone())).await
        .expect("Failed to send partial response");
    with_timeout(test_pair.server_stream.flush()).await
        .expect("Failed to flush partial response");
    
    let received_partial = with_timeout(test_pair.client_stream.read_exact(partial_response.len())).await
        .expect("Failed to read partial response");
    assert_eq!(received_partial, partial_response);
    println!("✓ Partial response received");
    
    // Step 3: Error condition
    println!("Step 3: Server encountering error...");
    let error_message = b"Processing failed after partial response".to_vec();
    let error_write_result = with_timeout(test_pair.server_stream.error_write(error_message.clone())).await;
    
    match error_write_result {
        Ok(_) => {
            println!("✓ Error written successfully");
            
            // Step 4: Client handling error
            println!("Step 4: Client handling error...");
            
            // Try normal read first (might get error)
            match with_timeout(test_pair.client_stream.read()).await {
                Ok(data) => {
                    if data.is_empty() {
                        println!("Got EOF on normal read");
                    } else {
                        println!("Got data on normal read: {} bytes", data.len());
                    }
                }
                Err(error_on_read) => {
                    if error_on_read.is_xstream_error() {
                        println!("✓ Got XStream error on normal read");
                    } else {
                        println!("Got IO error on normal read: {:?}", error_on_read.as_io_error());
                    }
                }
            }
            
            // Read error explicitly
            match with_timeout(test_pair.client_stream.error_read()).await {
                Ok(received_error) => {
                    println!("✓ Successfully read error: {:?}", String::from_utf8_lossy(&received_error));
                    assert_eq!(received_error, error_message);
                }
                Err(e) => {
                    println!("✗ Failed to read error: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Error write failed: {:?}", e);
        }
    }
    
    println!("✓ Complete error scenario diagnostics completed");
    with_timeout(shutdown_manager.shutdown()).await;
}
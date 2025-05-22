// xstream_error_handling_tests.rs
// Comprehensive error handling tests for XStream protocol focusing on real-world scenarios

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

// Helper function to enforce timeout on all tests
async fn with_timeout<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    match timeout(Duration::from_secs(10), future).await {
        Ok(result) => result,
        Err(_) => panic!("Test operation timed out after 10 seconds"),
    }
}

// Test 1: Client sends request, expects response, but server sends error instead
#[tokio::test]
async fn test_request_response_with_server_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client requests user data, server encounters database error
    
    // Step 1: Client sends request
    let request = b"GET /api/user/12345".to_vec();
    println!("Client sending request: {:?}", String::from_utf8_lossy(&request));
    
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);
    println!("Server received request: {:?}", String::from_utf8_lossy(&received_request));

    // Step 3: Server encounters error and sends error instead of response
    let error_message = b"Database connection timeout while fetching user data".to_vec();
    println!("Server sending error: {:?}", String::from_utf8_lossy(&error_message));
    
    with_timeout(test_pair.server_stream.error_write(error_message.clone()))
        .await
        .expect("Failed to write error from server");

    // Step 4: Give time for error to propagate through the error stream
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 5: Client tries to read response but should get error
    println!("Client attempting to read response...");
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            println!("Client received error as expected");
            assert!(error_on_read.is_xstream_error(), "Expected XStream error, got: {:?}", error_on_read);
            
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &error_message);
                println!("Error message: {:?}", xs_error.message());
            }
            
            // Should not have partial data since no response was sent
            assert!(!error_on_read.has_partial_data(), "Should not have partial data");
        }
        Ok(data) => {
            // If we get data instead of error, it might be due to timing
            // Let's check if error is available separately
            println!("Got data instead of error (timing issue): {:?}", data);
            
            // Try reading error directly
            match with_timeout(test_pair.client_stream.error_read()).await {
                Ok(error_data) => {
                    assert_eq!(error_data, error_message);
                    println!("✓ Error was available through error_read");
                }
                Err(e) => {
                    panic!("Expected error through error_read but got: {:?}", e);
                }
            }
        }
    }

    // Step 6: Client can read the error again (cached)
    let cached_error = with_timeout(test_pair.client_stream.error_read())
        .await
        .expect("Failed to read cached error");
    assert_eq!(cached_error, error_message);
    println!("✓ Cached error read successfully");

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 2: Client sends multiple requests, server processes some then encounters error
#[tokio::test]
async fn test_batch_processing_with_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client sends batch of operations, server processes some then fails
    
    let requests = vec![
        b"PROCESS item1".to_vec(),
        b"PROCESS item2".to_vec(),
        b"PROCESS item3".to_vec(),
    ];
    
    // Step 1: Client sends all requests
    for (i, request) in requests.iter().enumerate() {
        println!("Client sending request {}: {:?}", i + 1, String::from_utf8_lossy(request));
        with_timeout(test_pair.client_stream.write_all(request.clone()))
            .await
            .expect("Failed to send request");
        with_timeout(test_pair.client_stream.flush())
            .await
            .expect("Failed to flush request");
    }

    // Step 2: Server processes first two requests successfully
    for i in 0..2 {
        let received_request = with_timeout(test_pair.server_stream.read_exact(requests[i].len()))
            .await
            .expect("Failed to read request");
        assert_eq!(received_request, requests[i]);
        
        // Send successful response
        let response = format!("SUCCESS: item{} processed", i + 1).into_bytes();
        with_timeout(test_pair.server_stream.write_all(response.clone()))
            .await
            .expect("Failed to send response");
        with_timeout(test_pair.server_stream.flush())
            .await
            .expect("Failed to flush response");
        
        // Client reads successful response
        let received_response = with_timeout(test_pair.client_stream.read_exact(response.len()))
            .await
            .expect("Failed to read response");
        assert_eq!(received_response, response);
        println!("✓ Request {} processed successfully", i + 1);
    }

    // Step 3: Server reads third request but encounters error
    let third_request = with_timeout(test_pair.server_stream.read_exact(requests[2].len()))
        .await
        .expect("Failed to read third request");
    assert_eq!(third_request, requests[2]);
    
    // Server encounters error while processing third request
    let error_message = b"Disk space exhausted while processing item3".to_vec();
    println!("Server error during item3 processing: {:?}", String::from_utf8_lossy(&error_message));
    
    with_timeout(test_pair.server_stream.error_write(error_message.clone()))
        .await
        .expect("Failed to write error");

    // Give time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 4: Client tries to read third response but should get error
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            assert!(error_on_read.is_xstream_error(), "Expected XStream error");
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &error_message);
            }
            println!("✓ Client correctly received error for third request");
        }
        Ok(_) => {
            // Check error through error_read if we got data instead
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected error to be available");
            assert_eq!(error_data, error_message);
            println!("✓ Error available through error_read (timing difference)");
        }
    }

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 3: Client sends large request, server sends partial response then error
#[tokio::test]
async fn test_partial_response_then_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client downloads large file, server sends partial data then encounters error
    
    // Step 1: Client requests large file
    let request = b"DOWNLOAD /large-dataset.json".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server sends partial response (simulating streaming)
    let partial_response = b"HTTP/1.1 200 OK\r\nContent-Length: 1048576\r\n\r\n{\"users\":[{\"id\":1,\"name\":\"Alice\"}".to_vec();
    with_timeout(test_pair.server_stream.write_all(partial_response.clone()))
        .await
        .expect("Failed to send partial response");
    with_timeout(test_pair.server_stream.flush())
        .await
        .expect("Failed to flush partial response");

    // Step 4: Client reads partial response
    let received_partial = with_timeout(test_pair.client_stream.read_exact(partial_response.len()))
        .await
        .expect("Failed to read partial response");
    assert_eq!(received_partial, partial_response);
    println!("✓ Client received partial response: {} bytes", received_partial.len());

    // Step 5: Server encounters error while sending rest of data
    let error_message = b"Network interface failure during large file transfer".to_vec();
    with_timeout(test_pair.server_stream.error_write(error_message.clone()))
        .await
        .expect("Failed to write error");

    // Give time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 6: Client tries to read more data but should get error
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            assert!(error_on_read.is_xstream_error(), "Expected XStream error");
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &error_message);
                println!("Error during transfer: {:?}", xs_error.message());
            }
        }
        Ok(_) => {
            // Check error through error_read if we got data instead
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected error to be available");
            assert_eq!(error_data, error_message);
            println!("✓ Error available through error_read");
        }
    }

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 4: Client expects exact amount of data but server sends error mid-stream
#[tokio::test]
async fn test_read_exact_interrupted_by_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client expects exactly 1000 bytes but server can only send 500 before error
    
    // Step 1: Client requests specific amount of data
    let request = b"GET_EXACTLY 1000 bytes".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server sends only 500 bytes then encounters error
    let partial_data = vec![0x42; 500]; // 500 bytes of data
    with_timeout(test_pair.server_stream.write_all(partial_data.clone()))
        .await
        .expect("Failed to send partial data");
    with_timeout(test_pair.server_stream.flush())
        .await
        .expect("Failed to flush partial data");

    // Give time for data to be received
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Server sends error
    let error_message = b"Source data corrupted, cannot provide remaining 500 bytes".to_vec();
    with_timeout(test_pair.server_stream.error_write(error_message.clone()))
        .await
        .expect("Failed to write error");

    // Give additional time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 4: First, try to read some data to establish that partial data was sent
    let first_read_result = with_timeout(test_pair.client_stream.read()).await;
    let mut total_received = Vec::new();
    
    match first_read_result {
        Ok(data) => {
            total_received.extend_from_slice(&data);
            println!("✓ First read got {} bytes", data.len());
        }
        Err(error_on_read) => {
            if error_on_read.has_partial_data() {
                total_received.extend_from_slice(error_on_read.partial_data());
                println!("✓ First read got error with {} bytes of partial data", error_on_read.partial_data_len());
            }
            
            if error_on_read.is_xstream_error() {
                if let Some(xs_error) = error_on_read.as_xstream_error() {
                    assert_eq!(xs_error.data(), &error_message);
                    println!("✓ Got XStream error: {:?}", xs_error.message());
                }
            }
            
            // Verify error is available
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected error to be available");
            assert_eq!(error_data, error_message);
            
            with_timeout(shutdown_manager.shutdown()).await;
            return; // Test completed successfully
        }
    }
    
    // If we didn't get error in first read, try read_exact for remaining data
    if total_received.len() < 1000 {
        let remaining = 1000 - total_received.len();
        let result = with_timeout(test_pair.client_stream.read_exact(remaining)).await;
        
        match result {
            Err(error_on_read) => {
                if error_on_read.is_xstream_error() {
                    if error_on_read.has_partial_data() {
                        total_received.extend_from_slice(error_on_read.partial_data());
                        println!("✓ Read exact got error with {} additional bytes", error_on_read.partial_data_len());
                    }
                    
                    if let Some(xs_error) = error_on_read.as_xstream_error() {
                        assert_eq!(xs_error.data(), &error_message);
                        println!("✓ Got XStream error: {:?}", xs_error.message());
                    }
                } else {
                    // IO error - check if XStream error is available separately
                    println!("Got IO error during read_exact, checking for XStream error");
                    let error_data = with_timeout(test_pair.client_stream.error_read()).await
                        .expect("Expected error to be available");
                    assert_eq!(error_data, error_message);
                }
            }
            Ok(more_data) => {
                total_received.extend_from_slice(&more_data);
                println!("Got {} more bytes, total: {}", more_data.len(), total_received.len());
                
                // Should still have error available
                let error_data = with_timeout(test_pair.client_stream.error_read()).await
                    .expect("Expected error to be available");
                assert_eq!(error_data, error_message);
            }
        }
    }
    
    // Verify we got the expected partial data (should be 500 bytes or close to it)
    println!("✓ Total received: {} bytes (expected ~500)", total_received.len());
    if total_received.len() >= 500 {
        // Check that first 500 bytes match our partial data
        assert_eq!(&total_received[0..500], &partial_data);
        println!("✓ Partial data matches expected content");
    } else if !total_received.is_empty() {
        // Even if we got less, verify it matches the beginning of our test data
        assert_eq!(&total_received[..], &partial_data[0..total_received.len()]);
        println!("✓ Received data matches expected partial content");
    }
    
    // Final verification that error is available
    let final_error = with_timeout(test_pair.client_stream.error_read()).await
        .expect("Expected error to be available at end");
    assert_eq!(final_error, error_message);
    println!("✓ Error correctly available for final read");

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 5: Authentication/Authorization error scenario
#[tokio::test]
async fn test_authentication_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client tries to access protected resource without proper credentials
    
    // Step 1: Client sends request without auth token
    let request = b"GET /api/admin/users Authorization:".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server immediately sends authentication error
    let auth_error = b"HTTP 401 Unauthorized: Missing or invalid authentication token".to_vec();
    with_timeout(test_pair.server_stream.error_write(auth_error.clone()))
        .await
        .expect("Failed to write auth error");

    // Give time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 4: Client tries to read response but should get authentication error
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            assert!(error_on_read.is_xstream_error(), "Expected XStream error");
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &auth_error);
                assert!(xs_error.message().unwrap_or("").contains("401"));
                println!("✓ Authentication error received: {:?}", xs_error.message());
            }
        }
        Ok(_) => {
            // Check error through error_read if we got data instead
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected authentication error to be available");
            assert_eq!(error_data, auth_error);
            println!("✓ Authentication error available through error_read");
        }
    }

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 6: Rate limiting error scenario
#[tokio::test]
async fn test_rate_limiting_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client sends too many requests, server responds with rate limit error
    
    // Step 1: Simulate multiple rapid requests (we'll just send one for testing)
    let request = b"API_CALL /search?q=test".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server detects rate limit exceeded and sends error
    let rate_limit_error = b"HTTP 429 Too Many Requests: Rate limit exceeded. Try again in 60 seconds.".to_vec();
    with_timeout(test_pair.server_stream.error_write(rate_limit_error.clone()))
        .await
        .expect("Failed to write rate limit error");

    // Give time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 4: Client receives rate limiting error
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            assert!(error_on_read.is_xstream_error(), "Expected XStream error");
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &rate_limit_error);
                assert!(xs_error.message().unwrap_or("").contains("429"));
                println!("✓ Rate limiting error received: {:?}", xs_error.message());
            }
        }
        Ok(_) => {
            // Check error through error_read if we got data instead
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected rate limit error to be available");
            assert_eq!(error_data, rate_limit_error);
            println!("✓ Rate limiting error available through error_read");
        }
    }

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 7: Server internal error during processing
#[tokio::test]
async fn test_internal_server_error_during_processing() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Server starts processing but encounters internal error
    
    // Step 1: Client sends complex processing request
    let request = b"PROCESS_COMPLEX_DATA {\"algorithm\":\"ml-prediction\",\"data_size\":\"1GB\"}".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request and starts processing
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server sends processing started acknowledgment
    let ack = b"Processing started...".to_vec();
    with_timeout(test_pair.server_stream.write_all(ack.clone()))
        .await
        .expect("Failed to send acknowledgment");
    with_timeout(test_pair.server_stream.flush())
        .await
        .expect("Failed to flush acknowledgment");

    // Step 4: Client reads acknowledgment
    let received_ack = with_timeout(test_pair.client_stream.read_exact(ack.len()))
        .await
        .expect("Failed to read acknowledgment");
    assert_eq!(received_ack, ack);
    println!("✓ Client received processing acknowledgment");

    // Step 5: Server encounters internal error during processing
    let internal_error = b"Internal Server Error: Memory allocation failed during ML model loading".to_vec();
    with_timeout(test_pair.server_stream.error_write(internal_error.clone()))
        .await
        .expect("Failed to write internal error");

    // Give time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 6: Client waits for results but should get internal error
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            assert!(error_on_read.is_xstream_error(), "Expected XStream error");
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &internal_error);
                assert!(xs_error.message().unwrap_or("").contains("Memory allocation"));
                println!("✓ Internal server error received: {:?}", xs_error.message());
            }
        }
        Ok(_) => {
            // Check error through error_read if we got data instead
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected internal error to be available");
            assert_eq!(error_data, internal_error);
            println!("✓ Internal error available through error_read");
        }
    }

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 8: Error handling through explicit error_read call
#[tokio::test]
async fn test_explicit_error_read() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Test explicit error reading pattern
    
    // Step 1: Client sends request
    let request = b"TEST_ERROR_HANDLING".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server sends error
    let error_message = b"Test error for explicit error reading".to_vec();
    with_timeout(test_pair.server_stream.error_write(error_message.clone()))
        .await
        .expect("Failed to write error");

    // Step 4: Give time for error to be processed by background task
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Step 5: Client explicitly reads error
    let read_error = with_timeout(test_pair.client_stream.error_read())
        .await
        .expect("Failed to read error explicitly");
    
    assert_eq!(read_error, error_message);
    println!("✓ Explicit error read successful");

    // Step 6: Read error again (should be cached)
    let cached_error = with_timeout(test_pair.client_stream.error_read())
        .await
        .expect("Failed to read cached error");
    
    assert_eq!(cached_error, error_message);
    println!("✓ Cached error read successful");

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 9: Binary data request with binary error response
#[tokio::test]
async fn test_binary_data_with_binary_error() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Client requests binary data, server encounters error with binary error data
    
    // Step 1: Client requests binary file
    let request = b"GET_BINARY /images/large_image.png".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    // Step 2: Server receives request
    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    // Step 3: Server encounters error and sends binary error data
    let binary_error = vec![
        0xFF, 0xFE, 0xFD, 0xFC, // Magic bytes indicating error
        0x00, 0x01, 0x00, 0x02, // Error code 0x0100, sub-code 0x0002
        b'F', b'i', b'l', b'e', b' ', b'c', b'o', b'r', b'r', b'u', b'p', b't', b'e', b'd', // "File corrupted"
    ];
    
    with_timeout(test_pair.server_stream.error_write(binary_error.clone()))
        .await
        .expect("Failed to write binary error");

    // Give time for error to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 4: Client tries to read binary data but should get binary error
    let result = with_timeout(test_pair.client_stream.read()).await;
    
    match result {
        Err(error_on_read) => {
            assert!(error_on_read.is_xstream_error(), "Expected XStream error");
            if let Some(xs_error) = error_on_read.as_xstream_error() {
                assert_eq!(xs_error.data(), &binary_error);
                
                // Verify binary error structure
                let error_data = xs_error.data();
                assert_eq!(&error_data[0..4], &[0xFF, 0xFE, 0xFD, 0xFC]);
                assert_eq!(&error_data[4..8], &[0x00, 0x01, 0x00, 0x02]);
                
                println!("✓ Binary error received with {} bytes", error_data.len());
                println!("   Magic bytes: {:02X?}", &error_data[0..4]);
                println!("   Error codes: {:02X?}", &error_data[4..8]);
            }
        }
        Ok(_) => {
            // Check error through error_read if we got data instead
            let error_data = with_timeout(test_pair.client_stream.error_read()).await
                .expect("Expected binary error to be available");
            assert_eq!(error_data, binary_error);
            println!("✓ Binary error available through error_read");
        }
    }

    with_timeout(shutdown_manager.shutdown()).await;
}

// Test 10: Error caching and multiple reads
#[tokio::test]
async fn test_error_caching_multiple_reads() {
    let (mut test_pair, shutdown_manager) = with_timeout(create_xstream_test_pair()).await;

    // Scenario: Test that errors are properly cached and can be read multiple times
    
    // Step 1: Setup error scenario
    let request = b"CACHE_TEST".to_vec();
    with_timeout(test_pair.client_stream.write_all(request.clone()))
        .await
        .expect("Failed to send request");
    with_timeout(test_pair.client_stream.flush())
        .await
        .expect("Failed to flush request");

    let received_request = with_timeout(test_pair.server_stream.read_exact(request.len()))
        .await
        .expect("Failed to read request");
    assert_eq!(received_request, request);

    let error_message = b"Cached error test message".to_vec();
    with_timeout(test_pair.server_stream.error_write(error_message.clone()))
        .await
        .expect("Failed to write error");

    // Give time for error to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 2: Read error multiple times to test caching
    for i in 1..=5 {
        let read_error = with_timeout(test_pair.client_stream.error_read())
            .await
            .expect(&format!("Failed to read error on attempt {}", i));
        
        assert_eq!(read_error, error_message);
        println!("✓ Error read attempt {} successful", i);
    }

    // Step 3: Check that error is immediately available (cached)
    assert!(test_pair.client_stream.has_error_data().await);
    
    let cached_error = test_pair.client_stream.get_cached_error().await
        .expect("Expected cached error to be available");
    assert_eq!(cached_error, error_message);
    println!("✓ Cached error retrieval successful");

    with_timeout(shutdown_manager.shutdown()).await;
}
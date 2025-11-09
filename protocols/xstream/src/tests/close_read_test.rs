// src/tests/close_read_test.rs
// Test for close_read functionality

use crate::tests::xstream_tests::create_xstream_test_pair;

/// Test that close_read prevents further reads
#[tokio::test]
async fn test_close_read_functionality() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Write some data from client to server
    let test_data = vec![1, 2, 3, 4, 5];
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // Close read on server side
    test_pair.server_stream.close_read().await;

    // Server should not be able to read after close_read
    let read_result = test_pair.server_stream.read_exact(5).await;
    assert!(read_result.is_err(), "Server should not be able to read after close_read");

    // Client should still be able to write
    let write_result = test_pair.client_stream.write_all(vec![6, 7, 8]).await;
    assert!(write_result.is_ok(), "Client should still be able to write after server close_read");

    // But server should not be able to read the new data
    let read_result2 = test_pair.server_stream.read_exact(3).await;
    assert!(read_result2.is_err(), "Server should not be able to read new data after close_read");

    shutdown_manager.shutdown().await;
}

/// Test that close_read affects all clones
#[tokio::test]
async fn test_close_read_affects_clones() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Create a clone of server stream
    let server_clone = test_pair.server_stream.clone();

    // Close read on original
    test_pair.server_stream.close_read().await;

    // Clone should also be affected
    let read_result = server_clone.read_exact(1).await;
    assert!(read_result.is_err(), "Clone should not be able to read after close_read on original");

    shutdown_manager.shutdown().await;
}

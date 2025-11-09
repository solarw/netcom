// src/tests/strict_echo_test.rs
// Strict echo test with enhanced validation and immediate failure on any deviation

use crate::tests::{xstream_tests::create_xstream_test_pair, strict_test_utils};

/// Strict echo test with immediate failure on any deviation from expected behavior
#[tokio::test]
async fn test_strict_echo_with_validation() {
    // Create the test pair using existing infrastructure
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test data with predictable pattern for validation
    let test_data = strict_test_utils::create_test_data(512);
    let expected_checksum = strict_test_utils::calculate_checksum(&test_data);

    // Step 1: Client writes data to server with strict validation
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(test_data.clone()).await,
        "strict test client write_all",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "strict test client flush",
    );

    // Step 2: Server reads the data with strict validation
    let received_data = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(test_data.len()).await,
        "strict test server read_exact",
    );

    // Strict validation of received data
    strict_test_utils::assert_data_equal(&test_data, &received_data, "strict client->server data transfer");
    strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

    // Step 3: Server echoes the data back with strict validation
    strict_test_utils::assert_stream_success(
        test_pair.server_stream.write_all(received_data.clone()).await,
        "strict test server write_all",
    );

    strict_test_utils::assert_stream_success(
        test_pair.server_stream.flush().await,
        "strict test server flush",
    );

    // Step 4: Client reads the echoed data with strict validation
    let echoed_data = strict_test_utils::assert_stream_success(
        test_pair.client_stream.read_exact(test_data.len()).await,
        "strict test client read_exact echoed",
    );

    // Strict validation of echoed data
    strict_test_utils::assert_data_equal(&test_data, &echoed_data, "strict server->client echo");
    strict_test_utils::validate_data_integrity(&echoed_data, expected_checksum);

    // Additional state validation
    strict_test_utils::validate_condition(
        !test_pair.client_stream.is_closed(),
        "Client stream should not be closed after successful echo"
    );
    strict_test_utils::validate_condition(
        !test_pair.server_stream.is_closed(),
        "Server stream should not be closed after successful echo"
    );

    // Perform coordinated shutdown
    shutdown_manager.shutdown().await;
}

/// Test with various data sizes to ensure robustness
#[tokio::test]
async fn test_strict_echo_various_sizes() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test with small data
    let small_data = strict_test_utils::create_test_data(16);
    run_strict_echo_test(&test_pair, &small_data).await;

    // Test with medium data
    let medium_data = strict_test_utils::create_test_data(1024);
    run_strict_echo_test(&test_pair, &medium_data).await;

    // Test with large data
    let large_data = strict_test_utils::create_test_data(8192);
    run_strict_echo_test(&test_pair, &large_data).await;

    shutdown_manager.shutdown().await;
}

/// Helper function to run strict echo test
async fn run_strict_echo_test(test_pair: &crate::tests::xstream_tests::XStreamTestPair, test_data: &[u8]) {
    let expected_checksum = strict_test_utils::calculate_checksum(test_data);

    // Client writes data
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(test_data.to_vec()).await,
        "run_strict_echo client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "run_strict_echo client flush",
    );

    // Server reads data
    let received_data = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(test_data.len()).await,
        "run_strict_echo server read",
    );

    // Strict validation
    strict_test_utils::assert_data_equal(test_data, &received_data, "run_strict_echo data transfer");
    strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

    // Server echoes data
    strict_test_utils::assert_stream_success(
        test_pair.server_stream.write_all(received_data.clone()).await,
        "run_strict_echo server echo",
    );

    strict_test_utils::assert_stream_success(
        test_pair.server_stream.flush().await,
        "run_strict_echo server flush",
    );

    // Client reads echo
    let echoed_data = strict_test_utils::assert_stream_success(
        test_pair.client_stream.read_exact(test_data.len()).await,
        "run_strict_echo client read echo",
    );

    // Strict validation of echo
    strict_test_utils::assert_data_equal(test_data, &echoed_data, "run_strict_echo echo validation");
    strict_test_utils::validate_data_integrity(&echoed_data, expected_checksum);
}

/// Test error conditions with strict validation
#[tokio::test]
async fn test_strict_error_conditions() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test writing to closed stream (should fail)
    let test_data = strict_test_utils::create_test_data(64);

    // Close client stream first
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.close().await,
        "strict error test close client",
    );

    // Try to write to closed stream - should fail
    let write_result = test_pair.client_stream.write_all(test_data).await;
    strict_test_utils::validate_condition(
        write_result.is_err(),
        "Writing to closed stream should fail"
    );

    // Try to read from closed stream - should fail
    let read_result = test_pair.client_stream.read_exact(10).await;
    strict_test_utils::validate_condition(
        read_result.is_err(),
        "Reading from closed stream should fail"
    );

    // Verify stream state
    strict_test_utils::validate_condition(
        test_pair.client_stream.is_closed(),
        "Client stream should be closed after close()"
    );

    shutdown_manager.shutdown().await;
}

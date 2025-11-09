// src/tests/strict_boundary_tests.rs
// Strict boundary tests for XStream with enhanced validation

use crate::tests::{xstream_tests::create_xstream_test_pair, strict_test_utils};

/// Test boundary conditions with strict validation
#[tokio::test]
async fn test_strict_boundary_conditions() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test with empty data
    let empty_data = vec![];
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(empty_data.clone()).await,
        "boundary test empty write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "boundary test empty flush",
    );

    // Test with single byte
    let single_byte = vec![0x42];
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(single_byte.clone()).await,
        "boundary test single byte write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "boundary test single byte flush",
    );

    // Server should be able to read the single byte
    let received_byte = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(1).await,
        "boundary test single byte read",
    );

    strict_test_utils::assert_data_equal(&single_byte, &received_byte, "boundary test single byte transfer");

    shutdown_manager.shutdown().await;
}

/// Test concurrent operations with strict validation
#[tokio::test]
async fn test_strict_concurrent_operations() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Create test data
    let test_data = strict_test_utils::create_test_data(256);
    let expected_checksum = strict_test_utils::calculate_checksum(&test_data);

    // Concurrent write and read operations
    let client_write = test_pair.client_stream.write_all(test_data.clone());
    let server_read = test_pair.server_stream.read_exact(test_data.len());

    // Execute both operations
    let (write_result, read_result) = tokio::join!(client_write, server_read);

    // Strict validation of both operations
    strict_test_utils::assert_stream_success(write_result, "concurrent client write");
    let received_data = strict_test_utils::assert_stream_success(read_result, "concurrent server read");

    // Validate data integrity
    strict_test_utils::assert_data_equal(&test_data, &received_data, "concurrent data transfer");
    strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

    // Flush after concurrent operations
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "concurrent client flush",
    );

    shutdown_manager.shutdown().await;
}

/// Test stream state transitions with strict validation
#[tokio::test]
async fn test_strict_stream_state_transitions() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Initial state validation
    strict_test_utils::validate_condition(
        !test_pair.client_stream.is_closed(),
        "Client stream should not be closed initially"
    );
    strict_test_utils::validate_condition(
        !test_pair.server_stream.is_closed(),
        "Server stream should not be closed initially"
    );

    // Test write EOF
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_eof().await,
        "state test client write_eof",
    );

    // Validate state after write EOF
    strict_test_utils::validate_condition(
        test_pair.client_stream.is_write_local_closed(),
        "Client stream write should be closed after write_eof"
    );
    strict_test_utils::validate_condition(
        !test_pair.client_stream.is_closed(),
        "Client stream should not be fully closed after write_eof"
    );

    // Test closing one side
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.close().await,
        "state test client close",
    );

    // Validate state after close
    strict_test_utils::validate_condition(
        test_pair.client_stream.is_closed(),
        "Client stream should be closed after close()"
    );

    // Server should still be operational
    strict_test_utils::validate_condition(
        !test_pair.server_stream.is_closed(),
        "Server stream should still be operational after client close"
    );

    // Close server as well
    strict_test_utils::assert_stream_success(
        test_pair.server_stream.close().await,
        "state test server close",
    );

    // Final state validation
    strict_test_utils::validate_condition(
        test_pair.server_stream.is_closed(),
        "Server stream should be closed after close()"
    );

    shutdown_manager.shutdown().await;
}

/// Test error propagation with strict validation
#[tokio::test]
async fn test_strict_error_propagation() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test data
    let test_data = strict_test_utils::create_test_data(128);

    // Close client stream first
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.close().await,
        "error test close client",
    );

    // Attempt operations on closed stream - should fail
    let write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    strict_test_utils::validate_condition(
        write_result.is_err(),
        "Writing to closed stream should fail"
    );

    let read_result = test_pair.client_stream.read_exact(10).await;
    strict_test_utils::validate_condition(
        read_result.is_err(),
        "Reading from closed stream should fail"
    );

    let flush_result = test_pair.client_stream.flush().await;
    strict_test_utils::validate_condition(
        flush_result.is_err(),
        "Flushing closed stream should fail"
    );

    // Server should still be operational
    strict_test_utils::validate_condition(
        !test_pair.server_stream.is_closed(),
        "Server stream should still be operational"
    );

    // Test server operations after client close
    // С новым close_read() поведение изменилось:
    // - Клиент закрыл чтение (close_read()), поэтому не может читать данные
    // - Но сервер может продолжать писать, так как его WriteHalf все еще открыт
    // - В реальной сети это приведет к накоплению данных в буфере или ошибке на стороне транспорта
    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    strict_test_utils::validate_condition(
        server_write_result.is_ok(),
        "Server should be able to write after client close (data will be buffered or cause transport error)"
    );

    let server_flush_result = test_pair.server_stream.flush().await;
    strict_test_utils::validate_condition(
        server_flush_result.is_ok(),
        "Server should be able to flush after client close"
    );

    // Но клиент не должен быть способен читать данные после close_read()
    let client_read_result = test_pair.client_stream.read_exact(test_data.len()).await;
    strict_test_utils::validate_condition(
        client_read_result.is_err(),
        "Client should not be able to read after close_read()"
    );

    // Закрываем серверный поток для полной очистки
    strict_test_utils::assert_stream_success(
        test_pair.server_stream.close().await,
        "error test close server",
    );

    shutdown_manager.shutdown().await;
}

/// Test data integrity with various patterns
#[tokio::test]
async fn test_strict_data_integrity_patterns() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test with alternating pattern
    let alternating_data: Vec<u8> = (0..256).map(|i| if i % 2 == 0 { 0xAA } else { 0x55 }).collect();
    let alternating_checksum = strict_test_utils::calculate_checksum(&alternating_data);

    // Test with sequential pattern
    let sequential_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let sequential_checksum = strict_test_utils::calculate_checksum(&sequential_data);

    // Test alternating pattern
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(alternating_data.clone()).await,
        "pattern test alternating write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "pattern test alternating flush",
    );

    let received_alternating = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(alternating_data.len()).await,
        "pattern test alternating read",
    );

    strict_test_utils::assert_data_equal(&alternating_data, &received_alternating, "alternating pattern transfer");
    strict_test_utils::validate_data_integrity(&received_alternating, alternating_checksum);

    // Test sequential pattern
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(sequential_data.clone()).await,
        "pattern test sequential write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "pattern test sequential flush",
    );

    let received_sequential = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(sequential_data.len()).await,
        "pattern test sequential read",
    );

    strict_test_utils::assert_data_equal(&sequential_data, &received_sequential, "sequential pattern transfer");
    strict_test_utils::validate_data_integrity(&received_sequential, sequential_checksum);

    shutdown_manager.shutdown().await;
}

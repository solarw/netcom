// src/tests/strict_data_integrity_tests.rs
// Strict data integrity tests for XStream with comprehensive validation

use crate::tests::{xstream_tests::create_xstream_test_pair, strict_test_utils};

/// Test data integrity with large data transfers
#[tokio::test]
async fn test_strict_large_data_integrity() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test with large data (64KB)
    let large_data = strict_test_utils::create_test_data(64 * 1024);
    let expected_checksum = strict_test_utils::calculate_checksum(&large_data);

    // Client writes large data
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(large_data.clone()).await,
        "large data client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "large data client flush",
    );

    // Server reads large data
    let received_data = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(large_data.len()).await,
        "large data server read",
    );

    // Strict validation of large data
    strict_test_utils::assert_data_equal(&large_data, &received_data, "large data transfer");
    strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

    shutdown_manager.shutdown().await;
}

/// Test data integrity with multiple sequential transfers
#[tokio::test]
async fn test_strict_sequential_transfers_integrity() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Multiple sequential transfers
    let transfers = vec![
        strict_test_utils::create_test_data(1024),
        strict_test_utils::create_test_data(2048),
        strict_test_utils::create_test_data(4096),
        strict_test_utils::create_test_data(8192),
    ];

    let mut expected_checksums = Vec::new();

    // Send all transfers
    for (i, data) in transfers.iter().enumerate() {
        let checksum = strict_test_utils::calculate_checksum(data);
        expected_checksums.push(checksum);

        strict_test_utils::assert_stream_success(
            test_pair.client_stream.write_all(data.clone()).await,
            &format!("sequential transfer {} client write", i),
        );

        strict_test_utils::assert_stream_success(
            test_pair.client_stream.flush().await,
            &format!("sequential transfer {} client flush", i),
        );
    }

    // Receive and validate all transfers
    for (i, (expected_data, expected_checksum)) in transfers.iter().zip(expected_checksums.iter()).enumerate() {
        let received_data = strict_test_utils::assert_stream_success(
            test_pair.server_stream.read_exact(expected_data.len()).await,
            &format!("sequential transfer {} server read", i),
        );

        strict_test_utils::assert_data_equal(expected_data, &received_data, &format!("sequential transfer {}", i));
        strict_test_utils::validate_data_integrity(&received_data, *expected_checksum);
    }

    shutdown_manager.shutdown().await;
}

/// Test data integrity with interleaved reads and writes
#[tokio::test]
async fn test_strict_interleaved_transfers_integrity() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Create test data
    let data1 = strict_test_utils::create_test_data(1024);
    let data2 = strict_test_utils::create_test_data(2048);
    let data3 = strict_test_utils::create_test_data(512);

    let checksum1 = strict_test_utils::calculate_checksum(&data1);
    let checksum2 = strict_test_utils::calculate_checksum(&data2);
    let checksum3 = strict_test_utils::calculate_checksum(&data3);

    // Interleaved write-read pattern
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(data1.clone()).await,
        "interleaved data1 client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "interleaved data1 client flush",
    );

    // Read first data
    let received1 = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(data1.len()).await,
        "interleaved data1 server read",
    );

    strict_test_utils::assert_data_equal(&data1, &received1, "interleaved data1 transfer");
    strict_test_utils::validate_data_integrity(&received1, checksum1);

    // Write second data
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(data2.clone()).await,
        "interleaved data2 client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "interleaved data2 client flush",
    );

    // Read second data
    let received2 = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(data2.len()).await,
        "interleaved data2 server read",
    );

    strict_test_utils::assert_data_equal(&data2, &received2, "interleaved data2 transfer");
    strict_test_utils::validate_data_integrity(&received2, checksum2);

    // Write third data
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(data3.clone()).await,
        "interleaved data3 client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "interleaved data3 client flush",
    );

    // Read third data
    let received3 = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(data3.len()).await,
        "interleaved data3 server read",
    );

    strict_test_utils::assert_data_equal(&data3, &received3, "interleaved data3 transfer");
    strict_test_utils::validate_data_integrity(&received3, checksum3);

    shutdown_manager.shutdown().await;
}

/// Test data integrity with partial reads and writes
#[tokio::test]
async fn test_strict_partial_transfers_integrity() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Create test data
    let full_data = strict_test_utils::create_test_data(4096);
    let expected_checksum = strict_test_utils::calculate_checksum(&full_data);

    // Write data in chunks
    let chunk_size = 1024;
    for chunk in full_data.chunks(chunk_size) {
        strict_test_utils::assert_stream_success(
            test_pair.client_stream.write_all(chunk.to_vec()).await,
            "partial transfer client write chunk",
        );
    }

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "partial transfer client flush",
    );

    // Read data in chunks
    let mut received_data = Vec::new();
    let mut remaining = full_data.len();

    while remaining > 0 {
        let read_size = std::cmp::min(chunk_size, remaining);
        let chunk = strict_test_utils::assert_stream_success(
            test_pair.server_stream.read_exact(read_size).await,
            "partial transfer server read chunk",
        );
        received_data.extend_from_slice(&chunk);
        remaining -= read_size;
    }

    // Validate complete data
    strict_test_utils::assert_data_equal(&full_data, &received_data, "partial transfer complete data");
    strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

    shutdown_manager.shutdown().await;
}

/// Test data integrity with echo pattern (bidirectional transfer)
#[tokio::test]
async fn test_strict_bidirectional_echo_integrity() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test data for both directions
    let client_data = strict_test_utils::create_test_data(2048);
    let server_data = strict_test_utils::create_test_data(1024);

    let client_checksum = strict_test_utils::calculate_checksum(&client_data);
    let server_checksum = strict_test_utils::calculate_checksum(&server_data);

    // Client sends data to server
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(client_data.clone()).await,
        "bidirectional client->server write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "bidirectional client->server flush",
    );

    // Server sends data to client
    strict_test_utils::assert_stream_success(
        test_pair.server_stream.write_all(server_data.clone()).await,
        "bidirectional server->client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.server_stream.flush().await,
        "bidirectional server->client flush",
    );

    // Server reads client data
    let received_client_data = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(client_data.len()).await,
        "bidirectional server read client data",
    );

    // Client reads server data
    let received_server_data = strict_test_utils::assert_stream_success(
        test_pair.client_stream.read_exact(server_data.len()).await,
        "bidirectional client read server data",
    );

    // Validate both directions
    strict_test_utils::assert_data_equal(&client_data, &received_client_data, "bidirectional client->server");
    strict_test_utils::validate_data_integrity(&received_client_data, client_checksum);

    strict_test_utils::assert_data_equal(&server_data, &received_server_data, "bidirectional server->client");
    strict_test_utils::validate_data_integrity(&received_server_data, server_checksum);

    shutdown_manager.shutdown().await;
}

/// Test data integrity with specific patterns that might reveal issues
#[tokio::test]
async fn test_strict_pattern_based_integrity() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test patterns that might reveal buffer or encoding issues
    let patterns = vec![
        // All zeros
        vec![0u8; 1024],
        // All ones
        vec![1u8; 1024],
        // Alternating pattern
        (0..512).flat_map(|i| vec![0xAA, 0x55]).collect(),
        // Sequential pattern
        (0..256).map(|i| i as u8).collect::<Vec<u8>>(),
        // Random-like pattern (but deterministic)
        (0..1024).map(|i| ((i * 7) % 256) as u8).collect(),
    ];

    for (pattern_idx, pattern) in patterns.iter().enumerate() {
        let expected_checksum = strict_test_utils::calculate_checksum(pattern);

        strict_test_utils::assert_stream_success(
            test_pair.client_stream.write_all(pattern.clone()).await,
            &format!("pattern {} client write", pattern_idx),
        );

        strict_test_utils::assert_stream_success(
            test_pair.client_stream.flush().await,
            &format!("pattern {} client flush", pattern_idx),
        );

        let received_pattern = strict_test_utils::assert_stream_success(
            test_pair.server_stream.read_exact(pattern.len()).await,
            &format!("pattern {} server read", pattern_idx),
        );

        strict_test_utils::assert_data_equal(pattern, &received_pattern, &format!("pattern {} transfer", pattern_idx));
        strict_test_utils::validate_data_integrity(&received_pattern, expected_checksum);
    }

    shutdown_manager.shutdown().await;
}

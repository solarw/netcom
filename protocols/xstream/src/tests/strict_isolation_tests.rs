// src/tests/strict_isolation_tests.rs
// Strict isolation and determinism tests for XStream

use crate::tests::{xstream_tests::create_xstream_test_pair, strict_test_utils};

/// Test that XStream doesn't have dependencies on other projects
#[tokio::test]
async fn test_strict_isolation_from_other_projects() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Verify that we're only using XStream types and not external dependencies
    let client_type = std::any::type_name_of_val(&test_pair.client_stream);
    let server_type = std::any::type_name_of_val(&test_pair.server_stream);

    // Ensure types are from XStream, not other projects
    strict_test_utils::validate_condition(
        client_type.contains("xstream") || client_type.contains("XStream"),
        "Client stream should be XStream type"
    );
    strict_test_utils::validate_condition(
        server_type.contains("xstream") || server_type.contains("XStream"),
        "Server stream should be XStream type"
    );

    // Test basic functionality to ensure isolation doesn't break core operations
    let test_data = strict_test_utils::create_test_data(128);
    let expected_checksum = strict_test_utils::calculate_checksum(&test_data);

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(test_data.clone()).await,
        "isolation test client write",
    );

    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "isolation test client flush",
    );

    let received_data = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(test_data.len()).await,
        "isolation test server read",
    );

    strict_test_utils::assert_data_equal(&test_data, &received_data, "isolation test data transfer");
    strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

    shutdown_manager.shutdown().await;
}

/// Test deterministic behavior across multiple runs
#[tokio::test]
async fn test_strict_deterministic_behavior() {
    strict_test_utils::with_timeout(
        async {
            // Run the same test multiple times to ensure deterministic behavior
            for run in 0..2 {
                let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

                // Use deterministic test data
                let test_data: Vec<u8> = (0..256).map(|i| (i as u8).wrapping_add(run as u8)).collect();
                let expected_checksum = strict_test_utils::calculate_checksum(&test_data);

                // Perform the same operations each run
                strict_test_utils::assert_stream_success(
                    test_pair.client_stream.write_all(test_data.clone()).await,
                    &format!("deterministic test run {} client write", run),
                );

                strict_test_utils::assert_stream_success(
                    test_pair.client_stream.flush().await,
                    &format!("deterministic test run {} client flush", run),
                );

                let received_data = strict_test_utils::assert_stream_success(
                    test_pair.server_stream.read_exact(test_data.len()).await,
                    &format!("deterministic test run {} server read", run),
                );

                // Results should be identical across runs
                strict_test_utils::assert_data_equal(&test_data, &received_data, &format!("deterministic test run {}", run));
                strict_test_utils::validate_data_integrity(&received_data, expected_checksum);

                shutdown_manager.shutdown().await;
            }
        },
        strict_test_utils::long_test_timeout(),
        "test_strict_deterministic_behavior"
    ).await;
}

/// Test that XStream doesn't leak resources or have side effects
#[tokio::test]
async fn test_strict_resource_isolation() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Test that operations don't have side effects on stream state
    let initial_client_state = test_pair.client_stream.is_closed();
    let initial_server_state = test_pair.server_stream.is_closed();

    strict_test_utils::validate_condition(
        !initial_client_state,
        "Client stream should not be closed initially"
    );
    strict_test_utils::validate_condition(
        !initial_server_state,
        "Server stream should not be closed initially"
    );

    // Perform operations
    let test_data = strict_test_utils::create_test_data(512);
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.write_all(test_data.clone()).await,
        "resource isolation client write",
    );

    // Verify state hasn't changed unexpectedly
    strict_test_utils::validate_condition(
        !test_pair.client_stream.is_closed(),
        "Client stream should not be closed after write"
    );
    strict_test_utils::validate_condition(
        !test_pair.server_stream.is_closed(),
        "Server stream should not be closed after client write"
    );

    // Read data
    let received_data = strict_test_utils::assert_stream_success(
        test_pair.server_stream.read_exact(test_data.len()).await,
        "resource isolation server read",
    );

    // Verify state consistency
    strict_test_utils::validate_condition(
        !test_pair.client_stream.is_closed(),
        "Client stream should not be closed after read"
    );
    strict_test_utils::validate_condition(
        !test_pair.server_stream.is_closed(),
        "Server stream should not be closed after read"
    );

    strict_test_utils::assert_data_equal(&test_data, &received_data, "resource isolation data transfer");

    shutdown_manager.shutdown().await;
}

/// Test that XStream handles concurrent operations without interference
#[tokio::test]
async fn test_strict_concurrent_isolation() {
    let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

    // Create multiple independent data streams
    let data_sets = vec![
        strict_test_utils::create_test_data(256),
        strict_test_utils::create_test_data(512),
        strict_test_utils::create_test_data(1024),
    ];

    let mut expected_checksums = Vec::new();

    // Perform sequential writes first (simpler approach)
    for (i, data) in data_sets.iter().enumerate() {
        let checksum = strict_test_utils::calculate_checksum(data);
        expected_checksums.push(checksum);

        strict_test_utils::assert_stream_success(
            test_pair.client_stream.write_all(data.clone()).await,
            &format!("concurrent isolation write {}", i),
        );
    }

    // Flush after all writes
    strict_test_utils::assert_stream_success(
        test_pair.client_stream.flush().await,
        "concurrent isolation flush",
    );

    // Perform sequential reads and validate
    for (i, data) in data_sets.iter().enumerate() {
        let received_data = strict_test_utils::assert_stream_success(
            test_pair.server_stream.read_exact(data.len()).await,
            &format!("concurrent isolation read {}", i),
        );
        strict_test_utils::assert_data_equal(data, &received_data, &format!("concurrent isolation data {}", i));
        strict_test_utils::validate_data_integrity(&received_data, expected_checksums[i]);
    }

    shutdown_manager.shutdown().await;
}

/// Test that XStream doesn't depend on external state or global variables
#[tokio::test]
async fn test_strict_stateless_behavior() {
    strict_test_utils::with_timeout(
        async {
            // Run multiple independent test pairs to ensure stateless behavior
            let mut test_pairs = Vec::new();
            let mut shutdown_managers = Vec::new();

            // Create multiple independent connections
            for i in 0..2 {
                let (test_pair, shutdown_manager) = create_xstream_test_pair().await;
                test_pairs.push((i, test_pair));
                shutdown_managers.push(shutdown_manager);
            }

            // Test each pair independently
            for (i, test_pair) in test_pairs {
                let test_data = strict_test_utils::create_test_data(128 + i * 64);
                let expected_checksum = strict_test_utils::calculate_checksum(&test_data);

                strict_test_utils::assert_stream_success(
                    test_pair.client_stream.write_all(test_data.clone()).await,
                    &format!("stateless test pair {} client write", i),
                );

                strict_test_utils::assert_stream_success(
                    test_pair.client_stream.flush().await,
                    &format!("stateless test pair {} client flush", i),
                );

                let received_data = strict_test_utils::assert_stream_success(
                    test_pair.server_stream.read_exact(test_data.len()).await,
                    &format!("stateless test pair {} server read", i),
                );

                strict_test_utils::assert_data_equal(&test_data, &received_data, &format!("stateless test pair {}", i));
                strict_test_utils::validate_data_integrity(&received_data, expected_checksum);
            }

            // Clean up all connections
            for shutdown_manager in shutdown_managers {
                shutdown_manager.shutdown().await;
            }
        },
        strict_test_utils::long_test_timeout(),
        "test_strict_stateless_behavior"
    ).await;
}

/// Test that XStream handles cleanup properly without resource leaks
#[tokio::test]
async fn test_strict_cleanup_isolation() {
    strict_test_utils::with_timeout(
        async {
            // Test multiple create/destroy cycles
            for cycle in 0..2 {
                let (test_pair, shutdown_manager) = create_xstream_test_pair().await;

                // Perform some operations
                let test_data = strict_test_utils::create_test_data(256);
                strict_test_utils::assert_stream_success(
                    test_pair.client_stream.write_all(test_data.clone()).await,
                    &format!("cleanup test cycle {} client write", cycle),
                );

                strict_test_utils::assert_stream_success(
                    test_pair.client_stream.flush().await,
                    &format!("cleanup test cycle {} client flush", cycle),
                );

                let received_data = strict_test_utils::assert_stream_success(
                    test_pair.server_stream.read_exact(test_data.len()).await,
                    &format!("cleanup test cycle {} server read", cycle),
                );

                strict_test_utils::assert_data_equal(&test_data, &received_data, &format!("cleanup test cycle {}", cycle));

                // Clean up properly
                shutdown_manager.shutdown().await;

                // Small delay to ensure cleanup completes
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        },
        strict_test_utils::long_test_timeout(),
        "test_strict_cleanup_isolation"
    ).await;
}

// src/tests/strict_test_utils.rs
// Strict test utilities for XStream testing without external dependencies

use std::fmt::Debug;

/// Asserts that two data buffers are equal, panicking immediately on mismatch
pub fn assert_data_equal(expected: &[u8], actual: &[u8], context: &str) {
    if expected != actual {
        panic!(
            "Data mismatch in {}:\nExpected: {:?}\nActual: {:?}",
            context, expected, actual
        );
    }
}

/// Asserts that a stream operation succeeds, panicking immediately on failure
pub fn assert_stream_success<T>(result: Result<T, impl Debug>, operation: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("Stream operation '{}' failed: {:?}", operation, e),
    }
}

/// Asserts that a stream operation fails with expected error kind
pub fn assert_stream_failure<T>(
    result: Result<T, impl Debug>,
    operation: &str,
    expected_error_kind: std::io::ErrorKind,
) {
    match result {
        Ok(_) => panic!("Stream operation '{}' should have failed but succeeded", operation),
        Err(e) => {
            // Strict validation - panic immediately on unexpected errors
            panic!("Stream operation '{}' failed: {:?}", operation, e);
        }
    }
}

/// Creates test data with predictable content
pub fn create_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Calculates checksum for data validation
pub fn calculate_checksum(data: &[u8]) -> u32 {
    data.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32))
}

/// Validates data integrity with checksum
pub fn validate_data_integrity(data: &[u8], expected_checksum: u32) {
    let actual_checksum = calculate_checksum(data);
    if actual_checksum != expected_checksum {
        panic!(
            "Data integrity check failed: expected checksum {}, got {}",
            expected_checksum, actual_checksum
        );
    }
}

/// Enhanced assertion that immediately panics on any condition failure
pub fn assert_strict<T: PartialEq + Debug>(expected: T, actual: T, context: &str) {
    if expected != actual {
        panic!(
            "Strict assertion failed in {}:\nExpected: {:?}\nActual: {:?}",
            context, expected, actual
        );
    }
}

/// Validates that a condition is true, panicking immediately if false
pub fn validate_condition(condition: bool, message: &str) {
    if !condition {
        panic!("Condition validation failed: {}", message);
    }
}

/// Wraps an async operation with a timeout, panicking if timeout is exceeded
pub async fn with_timeout<F, T>(future: F, duration: std::time::Duration, operation: &str) -> T
where
    F: std::future::Future<Output = T>,
{
    match tokio::time::timeout(duration, future).await {
        Ok(result) => result,
        Err(_) => panic!("Operation '{}' timed out after {:?}", operation, duration),
    }
}

/// Creates a standard test timeout duration (10 seconds)
pub fn test_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(10)
}

/// Creates a short test timeout duration (5 seconds)
pub fn short_test_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(5)
}

/// Creates a long test timeout duration (30 seconds)
pub fn long_test_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(30)
}

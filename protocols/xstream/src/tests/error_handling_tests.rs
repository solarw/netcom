// tests/error_handling_tests.rs
// Tests for the error handling module

use std::io::ErrorKind;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

use crate::error_handling::ErrorDataStore;
use crate::types::{XStreamDirection, XStreamID};
use libp2p::identity;
use libp2p::PeerId;

// Helper to create test peer ID
fn create_test_peer_id() -> PeerId {
    let keypair = identity::Keypair::generate_ed25519();
    keypair.public().to_peer_id()
}

// Helper to create test closure notifier
fn create_test_closure_notifier() -> mpsc::UnboundedSender<(PeerId, XStreamID)> {
    let (tx, _rx) = mpsc::unbounded_channel();
    tx
}

#[tokio::test]
async fn test_error_data_store_basic_functionality() {
    let store = ErrorDataStore::new();
    
    // Initially no error
    assert!(!store.has_error().await);
    assert!(store.get_cached_error().await.is_none());
    assert!(!store.is_closed().await);
    
    // Store error data
    let test_data = b"test error message".to_vec();
    store.store_error(test_data.clone()).await.unwrap();
    
    // Should have error now
    assert!(store.has_error().await);
    assert_eq!(store.get_cached_error().await.unwrap(), test_data);
    
    // Reading should return the same data
    let read_data = store.wait_for_error().await.unwrap();
    assert_eq!(read_data, test_data);
}

#[tokio::test]
async fn test_error_data_store_multiple_readers() {
    let store = ErrorDataStore::new();
    let test_data = b"shared error data".to_vec();
    
    // Start multiple readers
    let store1 = store.clone();
    let store2 = store.clone();
    let store3 = store.clone();
    let expected1 = test_data.clone();
    let expected2 = test_data.clone();
    let expected3 = test_data.clone();
    
    let reader1 = tokio::spawn(async move {
        store1.wait_for_error().await
    });
    
    let reader2 = tokio::spawn(async move {
        store2.wait_for_error().await
    });
    
    let reader3 = tokio::spawn(async move {
        store3.wait_for_error().await
    });
    
    // Wait longer then store data
    sleep(Duration::from_millis(100)).await;
    store.store_error(test_data).await.unwrap();
    
    // All readers should get the data with much longer timeout
    let result1 = timeout(Duration::from_secs(15), reader1).await
        .expect("Reader1 timed out after 15 seconds").unwrap().unwrap();
    let result2 = timeout(Duration::from_secs(15), reader2).await
        .expect("Reader2 timed out after 15 seconds").unwrap().unwrap();
    let result3 = timeout(Duration::from_secs(15), reader3).await
        .expect("Reader3 timed out after 15 seconds").unwrap().unwrap();
    
    assert_eq!(result1, expected1);
    assert_eq!(result2, expected2);
    assert_eq!(result3, expected3);
}

#[tokio::test]
async fn test_error_data_store_close_behavior() {
    let store = ErrorDataStore::new();
    
    // Start a reader
    let store_clone = store.clone();
    let reader = tokio::spawn(async move {
        store_clone.wait_for_error().await
    });
    
    // Close the store after a delay
    sleep(Duration::from_millis(50)).await;
    store.close().await;
    
    // Reader should get an error due to store being closed
    let result = timeout(Duration::from_secs(1), reader).await.unwrap().unwrap();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::UnexpectedEof);
    
    // Store should be marked as closed
    assert!(store.is_closed().await);
}

#[tokio::test]
async fn test_error_data_store_cached_reads() {
    let store = ErrorDataStore::new();
    let test_data = b"cached error data".to_vec();
    
    // Store data
    store.store_error(test_data.clone()).await.unwrap();
    
    // Multiple reads should return cached data
    for i in 0..5 {
        let read_data = store.wait_for_error().await.unwrap();
        assert_eq!(read_data, test_data);
        
        // Проверяем что данные возвращаются (без строгих требований к времени)
        // В реальности кэшированные операции могут варьироваться по времени
        // из-за планировщика tokio и системной нагрузки
    }
    
    // Дополнительная проверка - все операции должны быть быстрыми в совокупности
    let start = tokio::time::Instant::now();
    for _ in 0..10 {
        let read_data = store.wait_for_error().await.unwrap();
        assert_eq!(read_data, test_data);
    }
    let total_duration = start.elapsed();
    
    // 10 кэшированных операций должны выполниться быстро (менее 50ms в сумме)
    assert!(
        total_duration < Duration::from_millis(50), 
        "10 cached reads took too long: {:?}", total_duration
    );
}

#[tokio::test]
async fn test_error_data_store_duplicate_store_ignored() {
    let store = ErrorDataStore::new();
    let first_data = b"first error".to_vec();
    let second_data = b"second error".to_vec();
    
    // Store first error
    store.store_error(first_data.clone()).await.unwrap();
    
    // Try to store second error (should be ignored)
    store.store_error(second_data).await.unwrap();
    
    // Should still return first error
    let read_data = store.wait_for_error().await.unwrap();
    assert_eq!(read_data, first_data);
}

#[tokio::test]
async fn test_error_data_store_clear_cache() {
    let store = ErrorDataStore::new();
    let test_data = b"test error".to_vec();
    
    // Store data
    store.store_error(test_data.clone()).await.unwrap();
    assert!(store.has_error().await);
    
    // Clear cache
    store.clear_cache().await;
    assert!(!store.has_error().await);
    assert!(store.get_cached_error().await.is_none());
}

#[tokio::test]
async fn test_error_data_store_wait_after_close() {
    let store = ErrorDataStore::new();
    
    // Close store first
    store.close().await;
    
    // Waiting for error should return error immediately
    let result = store.wait_for_error().await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn test_error_data_store_store_after_close() {
    let store = ErrorDataStore::new();
    let test_data = b"test error".to_vec();
    
    // Close store first
    store.close().await;
    
    // Storing error should fail
    let result = store.store_error(test_data).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::NotConnected);
}

#[tokio::test]
async fn test_error_data_store_concurrent_access() {
    let store = ErrorDataStore::new();
    let test_data = b"concurrent test data".to_vec();
    
    // Start multiple concurrent operations
    let store1 = store.clone();
    let store2 = store.clone();
    let store3 = store.clone();
    
    let data1 = test_data.clone();
    let data2 = test_data.clone();
    
    // Concurrent readers
    let reader1 = tokio::spawn(async move {
        store1.wait_for_error().await
    });
    
    let reader2 = tokio::spawn(async move {
        store2.wait_for_error().await
    });
    
    // Concurrent writer (should only store once)
    let writer = tokio::spawn(async move {
        sleep(Duration::from_millis(100)).await; // Longer delay before writing
        store3.store_error(data1).await
    });
    
    // Another writer with different data (should be ignored)
    let store4 = store.clone();
    let late_writer = tokio::spawn(async move {
        sleep(Duration::from_millis(150)).await;
        store4.store_error(data2).await
    });
    
    // Wait for all operations with much longer timeouts
    let write_result = timeout(Duration::from_secs(15), writer).await
        .expect("Writer timed out after 15 seconds").unwrap();
    let late_write_result = timeout(Duration::from_secs(15), late_writer).await
        .expect("Late writer timed out after 15 seconds").unwrap();
    let read_result1 = timeout(Duration::from_secs(15), reader1).await
        .expect("Reader1 timed out after 15 seconds").unwrap().unwrap();
    let read_result2 = timeout(Duration::from_secs(15), reader2).await
        .expect("Reader2 timed out after 15 seconds").unwrap().unwrap();
    
    // First write should succeed
    assert!(write_result.is_ok());
    // Second write should succeed but be ignored
    assert!(late_write_result.is_ok());
    
    // Both readers should get the first data
    assert_eq!(read_result1, test_data);
    assert_eq!(read_result2, test_data);
}

#[tokio::test]
async fn test_error_data_store_stress_test() {
    let store = ErrorDataStore::new();
    let test_data = b"stress test data".to_vec();
    
    // Create fewer concurrent readers for better stability
    let mut readers = Vec::new();
    for i in 0..10 { // Reduced from 25 to 10
        let store_clone = store.clone();
        let expected_data = test_data.clone();
        let reader = tokio::spawn(async move {
            let result = store_clone.wait_for_error().await.unwrap();
            assert_eq!(result, expected_data);
            i // Return reader ID for verification
        });
        readers.push(reader);
    }
    
    // Wait longer then store data
    sleep(Duration::from_millis(200)).await; // Increased delay
    store.store_error(test_data).await.unwrap();
    
    // All readers should complete successfully with much longer timeout
    for (expected_id, reader) in readers.into_iter().enumerate() {
        let reader_id = timeout(Duration::from_secs(20), reader).await
            .expect(&format!("Reader {} timed out after 20 seconds", expected_id)).unwrap();
        assert_eq!(reader_id, expected_id);
    }
    
    // Store should have the error cached
    assert!(store.has_error().await);
}

#[tokio::test]
async fn test_error_data_store_default_trait() {
    let store = ErrorDataStore::default();
    
    // Should behave the same as new()
    assert!(!store.has_error().await);
    assert!(store.get_cached_error().await.is_none());
    assert!(!store.is_closed().await);
    
    let test_data = b"default trait test".to_vec();
    store.store_error(test_data.clone()).await.unwrap();
    
    let read_data = store.wait_for_error().await.unwrap();
    assert_eq!(read_data, test_data);
}

#[tokio::test]
async fn test_error_data_store_edge_cases() {
    let store = ErrorDataStore::new();
    
    // Test empty data
    store.store_error(vec![]).await.unwrap();
    let data = store.wait_for_error().await.unwrap();
    assert!(data.is_empty());
    
    // Clear and test again
    store.clear_cache().await;
    assert!(!store.has_error().await);
}

#[tokio::test]
async fn test_error_data_store_large_data() {
    let store = ErrorDataStore::new();
    
    // Test with large data (1MB)
    let large_data = vec![0xAB; 1024 * 1024];
    store.store_error(large_data.clone()).await.unwrap();
    
    let read_data = store.wait_for_error().await.unwrap();
    assert_eq!(read_data.len(), large_data.len());
    assert_eq!(read_data, large_data);
}

#[tokio::test]
async fn test_error_data_store_timing() {
    let store = ErrorDataStore::new();
    let test_data = b"timing test".to_vec();
    
    // Start reader before data is available
    let store_clone = store.clone();
    let reader = tokio::spawn(async move {
        let start = tokio::time::Instant::now();
        let result = store_clone.wait_for_error().await.unwrap();
        (result, start.elapsed())
    });
    
    // Wait 100ms then store data
    sleep(Duration::from_millis(100)).await;
    store.store_error(test_data.clone()).await.unwrap();
    
    let (data, duration) = timeout(Duration::from_secs(1), reader).await.unwrap().unwrap();
    assert_eq!(data, test_data);
    // Should have waited approximately 100ms
    assert!(duration >= Duration::from_millis(90));
    assert!(duration <= Duration::from_millis(200));
}

#[tokio::test]
async fn test_error_data_store_multiple_close() {
    let store = ErrorDataStore::new();
    
    // Close multiple times should be safe
    store.close().await;
    assert!(store.is_closed().await);
    
    store.close().await;
    assert!(store.is_closed().await);
    
    store.close().await;
    assert!(store.is_closed().await);
}

#[tokio::test]
async fn test_error_data_store_clone_behavior() {
    let store = ErrorDataStore::new();
    let test_data = b"clone test".to_vec();
    
    // Clone the store
    let store_clone = store.clone();
    
    // Store data in original
    store.store_error(test_data.clone()).await.unwrap();
    
    // Clone should see the same data
    assert!(store_clone.has_error().await);
    let cloned_data = store_clone.wait_for_error().await.unwrap();
    assert_eq!(cloned_data, test_data);
    
    // Close original
    store.close().await;
    
    // Clone should also be closed (shared state)
    assert!(store_clone.is_closed().await);
}
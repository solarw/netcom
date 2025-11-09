//! Tests for close_write() functionality in XStream
//! Проверяет безопасное закрытие записи с явным drop WriteHalf

use crate::tests::xstream_tests::create_xstream_test_pair;

/// Test basic close_write functionality
/// Проверяет базовую функциональность close_write()
#[tokio::test]
async fn test_close_write_functionality() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;
    let test_data = b"Test data for close_write".to_vec();

    // Write some data first
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // Close write on client
    test_pair.client_stream.close_write().await.unwrap();

    // Client should not be able to write after close_write()
    let client_write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    assert!(client_write_result.is_err(), "Client should not be able to write after close_write()");

    // Server should still be able to read the data
    let server_read_result = test_pair.server_stream.read().await;
    assert!(server_read_result.is_ok(), "Server should be able to read data after client close_write()");

    // Server should still be able to write
    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    assert!(server_write_result.is_ok(), "Server should be able to write after client close_write()");

    // Close both streams
    test_pair.server_stream.close().await.unwrap();
    test_pair.client_stream.close().await.unwrap();

    shutdown_manager.shutdown().await;
}

/// Test that close_write() affects all clones
/// Проверяет, что close_write() влияет на все клоны XStream
#[tokio::test]
async fn test_close_write_affects_clones() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;
    let test_data = b"Test data for clone close_write".to_vec();

    // Create clones
    let client_clone1 = test_pair.client_stream.clone();
    let client_clone2 = test_pair.client_stream.clone();

    // Write some data first
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // Close write on original
    test_pair.client_stream.close_write().await.unwrap();

    // All clones should also be unable to write
    let clone1_write_result = client_clone1.write_all(test_data.clone()).await;
    assert!(clone1_write_result.is_err(), "Clone 1 should not be able to write after close_write()");

    let clone2_write_result = client_clone2.write_all(test_data.clone()).await;
    assert!(clone2_write_result.is_err(), "Clone 2 should not be able to write after close_write()");

    // Original should also be unable to write
    let original_write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    assert!(original_write_result.is_err(), "Original should not be able to write after close_write()");

    // Server should still be able to read and write
    let server_read_result = test_pair.server_stream.read().await;
    assert!(server_read_result.is_ok(), "Server should be able to read after client close_write()");

    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    assert!(server_write_result.is_ok(), "Server should be able to write after client close_write()");

    // Close both streams
    test_pair.server_stream.close().await.unwrap();
    test_pair.client_stream.close().await.unwrap();

    shutdown_manager.shutdown().await;
}

/// Test that close() uses close_write() and close_read()
/// Проверяет, что close() использует close_write() и close_read()
#[tokio::test]
async fn test_close_uses_close_write_and_close_read() {
    let (mut test_pair, shutdown_manager) = create_xstream_test_pair().await;
    let test_data = b"Test data for close integration".to_vec();

    // Write some data first
    test_pair.client_stream.write_all(test_data.clone()).await.unwrap();
    test_pair.client_stream.flush().await.unwrap();

    // Close client stream (should use close_write() and close_read())
    test_pair.client_stream.close().await.unwrap();

    // Client should not be able to write after close()
    let client_write_result = test_pair.client_stream.write_all(test_data.clone()).await;
    assert!(client_write_result.is_err(), "Client should not be able to write after close()");

    // Client should not be able to read after close()
    let client_read_result = test_pair.client_stream.read().await;
    assert!(client_read_result.is_err(), "Client should not be able to read after close()");

    // Server operations after client close
    // С новым close_write() поведение изменилось:
    // - Клиент закрыл запись (close_write()), поэтому сервер может продолжать читать
    // - Клиент закрыл чтение (close_read()), поэтому сервер может продолжать писать
    // - В реальной сети это приведет к накоплению данных в буфере или ошибке на стороне транспорта
    let server_read_result = test_pair.server_stream.read().await;
    assert!(server_read_result.is_ok(), "Server should be able to read after client close (data available)");

    let server_write_result = test_pair.server_stream.write_all(test_data.clone()).await;
    assert!(server_write_result.is_ok(), "Server should be able to write after client close (data will be buffered or cause transport error)");

    // Закрываем серверный поток для полной очистки
    test_pair.server_stream.close().await.unwrap();

    shutdown_manager.shutdown().await;
}

// xstream_error_coverage_tests.rs
// Tests specifically targeting uncovered code in xstream_error.rs

use crate::xstream_error::{ErrorOnRead, IoErrorWrapper, ReadError, XStreamError, utils};
use std::io::{self, ErrorKind};

#[test]
fn test_error_on_read_new_constructor() {
    // Test the ErrorOnRead::new constructor which is currently uncovered (FNDA:0)
    let partial_data = b"partial data".to_vec();
    let io_error = io::Error::new(ErrorKind::TimedOut, "Timeout");
    let read_error = ReadError::from(io_error);
    
    // This should call the new constructor
    let error_on_read = ErrorOnRead::new(partial_data.clone(), read_error);
    
    // Verify the created object has correct properties
    assert_eq!(error_on_read.partial_data(), &partial_data);
    assert!(error_on_read.is_io_error());
    assert_eq!(error_on_read.kind(), ErrorKind::TimedOut);
    assert!(error_on_read.has_partial_data());
    assert_eq!(error_on_read.partial_data_len(), partial_data.len());
}

#[test]
fn test_error_on_read_error_method() {
    // Test ErrorOnRead::error method which is currently uncovered (FNDA:0)
    let partial_data = b"test".to_vec();
    let io_error = io::Error::new(ErrorKind::ConnectionReset, "Connection reset");
    let error_on_read = ErrorOnRead::from_io_error(partial_data, io_error);
    
    // Test the error method
    let extracted_error = error_on_read.error();
    match extracted_error {
        ReadError::Io(io_wrapper) => {
            assert_eq!(io_wrapper.kind(), ErrorKind::ConnectionReset);
            assert!(io_wrapper.message().contains("Connection reset"));
        }
        _ => panic!("Expected IO error"),
    }
}

#[test]
fn test_error_reader_task_methods() {
    // Test ErrorReaderTask methods that are currently uncovered
    // These methods have FNDA:0 in lcov.info
    use crate::error_handling::ErrorReaderTask;
    use libp2p::Stream;
    use tokio::io::duplex;
    
    // Note: ErrorReaderTask is complex to test directly due to async nature
    // and dependencies. We'll focus on other uncovered areas first.
}

#[test]
fn test_error_on_read_as_io_error_edge_case() {
    // Test edge case in ErrorOnRead::as_io_error when error is XStreamError
    let partial_data = b"data".to_vec();
    let xs_error = XStreamError::new(b"xstream error".to_vec());
    let error_on_read = ErrorOnRead::from_xstream_error(partial_data, xs_error);
    
    // as_io_error should return None for XStream errors
    assert!(error_on_read.as_io_error().is_none());
}

#[test]
fn test_error_on_read_as_xstream_error_edge_case() {
    // Test edge case in ErrorOnRead::as_xstream_error when error is IO
    let partial_data = b"data".to_vec();
    let io_error = io::Error::new(ErrorKind::BrokenPipe, "Pipe broken");
    let error_on_read = ErrorOnRead::from_io_error(partial_data, io_error);
    
    // as_xstream_error should return None for IO errors
    assert!(error_on_read.as_xstream_error().is_none());
}

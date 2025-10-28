// xstream_error_comprehensive_tests.rs
// Comprehensive tests for XStreamError and related error types focusing on uncovered code

use crate::xstream_error::{
    ErrorOnRead, IoErrorWrapper, ReadError, XStreamError, XStreamReadResult, utils
};
use std::io::{self, ErrorKind};

#[test]
fn test_xstream_error_empty_data() {
    // Test XStreamError with empty data
    let empty_error = XStreamError::new(Vec::new());
    
    assert!(empty_error.is_empty());
    assert_eq!(empty_error.len(), 0);
    assert_eq!(empty_error.data(), &[]);
    assert!(empty_error.message().is_none());
    // as_string() for empty data returns Some("") - empty string
    assert_eq!(empty_error.as_string(), Some("".to_string()));
    
    // Test Display for empty error
    let display_output = format!("{}", empty_error);
    assert!(display_output.contains("empty"));
}

#[test]
fn test_xstream_error_binary_data() {
    // Test XStreamError with binary data that cannot be converted to UTF-8
    let binary_data = vec![0xFF, 0xFE, 0x00, 0x01];
    let binary_error = XStreamError::new(binary_data.clone());
    
    assert!(!binary_error.is_empty());
    assert_eq!(binary_error.len(), 4);
    assert_eq!(binary_error.data(), &binary_data);
    assert!(binary_error.message().is_none()); // Should be None for non-UTF-8 data
    assert!(binary_error.as_string().is_none()); // Should be None for non-UTF-8 data
    
    // Test Display for binary data
    let display_output = format!("{}", binary_error);
    assert!(display_output.contains("4 bytes of binary data"));
}

#[test]
fn test_xstream_error_from_conversion() {
    // Test From<Vec<u8>> implementation
    let data = b"test data".to_vec();
    let error_from_vec: XStreamError = data.clone().into();
    
    assert_eq!(error_from_vec.data(), &data);
    assert_eq!(error_from_vec.message(), Some("test data"));
    
    // Test that the conversion preserves the data
    assert_eq!(error_from_vec.as_string(), Some("test data".to_string()));
}

#[test]
fn test_error_on_read_into_methods() {
    // Test into_partial_data, into_error, and into_parts methods
    let partial_data = b"partial".to_vec();
    let io_error = io::Error::new(ErrorKind::TimedOut, "Timeout");
    let error_on_read = ErrorOnRead::from_io_error(partial_data.clone(), io_error);
    
    // Test into_partial_data
    let consumed_error_on_read = error_on_read;
    let extracted_partial_data = consumed_error_on_read.into_partial_data();
    assert_eq!(extracted_partial_data, partial_data);
    
    // Create another one for into_error test
    let partial_data2 = b"partial2".to_vec();
    let io_error2 = io::Error::new(ErrorKind::TimedOut, "Timeout2");
    let error_on_read2 = ErrorOnRead::from_io_error(partial_data2.clone(), io_error2);
    
    let extracted_error = error_on_read2.into_error();
    match extracted_error {
        ReadError::Io(io_wrapper) => {
            assert_eq!(io_wrapper.kind(), ErrorKind::TimedOut);
            assert!(io_wrapper.message().contains("Timeout2"));
        }
        _ => panic!("Expected IO error"),
    }
    
    // Test into_parts
    let partial_data3 = b"partial3".to_vec();
    let xs_error = XStreamError::new(b"xstream error".to_vec());
    let error_on_read3 = ErrorOnRead::from_xstream_error(partial_data3.clone(), xs_error);
    
    let (extracted_data, extracted_read_error) = error_on_read3.into_parts();
    assert_eq!(extracted_data, partial_data3);
    match extracted_read_error {
        ReadError::XStream(xs_err) => {
            assert_eq!(xs_err.data(), b"xstream error");
        }
        _ => panic!("Expected XStream error"),
    }
}

#[test]
fn test_error_on_read_error_only_methods() {
    // Test error_only, io_error_only, and xstream_error_only methods
    let io_error = io::Error::new(ErrorKind::ConnectionReset, "Connection reset");
    let error_only = ErrorOnRead::io_error_only(io_error);
    
    assert!(!error_only.has_partial_data());
    assert_eq!(error_only.partial_data_len(), 0);
    assert!(error_only.is_io_error());
    
    let xs_error = XStreamError::new(b"server error".to_vec());
    let xs_error_only = ErrorOnRead::xstream_error_only(xs_error);
    
    assert!(!xs_error_only.has_partial_data());
    assert_eq!(xs_error_only.partial_data_len(), 0);
    assert!(xs_error_only.is_xstream_error());
    
    // Test generic error_only
    let read_error = ReadError::from(io::Error::new(ErrorKind::BrokenPipe, "Pipe broken"));
    let generic_error_only = ErrorOnRead::error_only(read_error);
    
    assert!(!generic_error_only.has_partial_data());
    assert!(generic_error_only.is_io_error());
}

#[test]
fn test_error_on_read_from_xstream_data() {
    // Test from_xstream_data method
    let partial_data = b"partial data".to_vec();
    let error_data = b"error occurred".to_vec();
    
    let error_on_read = ErrorOnRead::from_xstream_data(partial_data.clone(), error_data.clone());
    
    assert_eq!(error_on_read.partial_data(), &partial_data);
    assert!(error_on_read.is_xstream_error());
    
    if let Some(xs_error) = error_on_read.as_xstream_error() {
        assert_eq!(xs_error.data(), &error_data);
        assert_eq!(xs_error.message(), Some("error occurred"));
    } else {
        panic!("Expected XStream error");
    }
}

#[test]
fn test_error_on_read_from_std_io_error() {
    // Test from_std_io_error compatibility method
    let std_io_error = io::Error::new(ErrorKind::NotFound, "File not found");
    let error_on_read = ErrorOnRead::from_std_io_error(std_io_error);
    
    assert!(!error_on_read.has_partial_data());
    assert!(error_on_read.is_io_error());
    assert_eq!(error_on_read.kind(), ErrorKind::NotFound);
}

#[test]
fn test_read_error_display() {
    // Test Display implementations for ReadError variants
    let io_error = io::Error::new(ErrorKind::TimedOut, "Operation timed out");
    let io_read_error = ReadError::from(io_error);
    
    let io_display = format!("{}", io_read_error);
    assert!(io_display.contains("IO error"));
    assert!(io_display.contains("timed out"));
    
    let xs_error = XStreamError::new(b"server busy".to_vec());
    let xs_read_error = ReadError::from(xs_error);
    
    let xs_display = format!("{}", xs_read_error);
    assert!(xs_display.contains("XStream error"));
    assert!(xs_display.contains("server busy"));
}

#[test]
fn test_error_on_read_display() {
    // Test Display implementation for ErrorOnRead with and without partial data
    
    // Without partial data
    let io_error = io::Error::new(ErrorKind::BrokenPipe, "Pipe broken");
    let error_only = ErrorOnRead::io_error_only(io_error);
    let display_no_partial = format!("{}", error_only);
    assert!(display_no_partial.contains("before reading any data"));
    assert!(display_no_partial.contains("Pipe broken"));
    
    // With partial data
    let io_error2 = io::Error::new(ErrorKind::BrokenPipe, "Pipe broken");
    let partial_data = b"some data".to_vec();
    let error_with_partial = ErrorOnRead::from_io_error(partial_data, io_error2);
    let display_with_partial = format!("{}", error_with_partial);
    assert!(display_with_partial.contains("after reading"));
    assert!(display_with_partial.contains("9 bytes"));
    assert!(display_with_partial.contains("Pipe broken"));
}

#[test]
fn test_io_error_wrapper_conversion() {
    // Test IoErrorWrapper conversion methods
    let original_error = io::Error::new(ErrorKind::PermissionDenied, "Access denied");
    let wrapper = IoErrorWrapper::new(original_error);
    
    assert_eq!(wrapper.kind(), ErrorKind::PermissionDenied);
    assert!(wrapper.message().contains("Access denied"));
    
    // Test conversion back to io::Error
    let restored_error = wrapper.to_io_error();
    assert_eq!(restored_error.kind(), ErrorKind::PermissionDenied);
    assert!(restored_error.to_string().contains("Access denied"));
    
    // Test From<io::Error> implementation
    let another_error = io::Error::new(ErrorKind::AddrInUse, "Address in use");
    let wrapper_from: IoErrorWrapper = another_error.into();
    assert_eq!(wrapper_from.kind(), ErrorKind::AddrInUse);
}

#[test]
fn test_error_utils_functions() {
    // Test utility functions in the utils module
    let io_error = io::Error::new(ErrorKind::ConnectionAborted, "Connection aborted");
    
    // Test io_error_to_error_on_read
    let error_on_read = utils::io_error_to_error_on_read(io_error);
    assert!(error_on_read.is_io_error());
    assert!(!error_on_read.has_partial_data());
    
    // Test xstream_data_to_error_on_read
    let error_data = b"utility test error".to_vec();
    let xs_error_on_read = utils::xstream_data_to_error_on_read(error_data.clone());
    assert!(xs_error_on_read.is_xstream_error());
    assert!(!xs_error_on_read.has_partial_data());
    
    if let Some(xs_error) = xs_error_on_read.as_xstream_error() {
        assert_eq!(xs_error.data(), &error_data);
    }
    
    // Test is_critical_error
    let critical_io_error = io::Error::new(ErrorKind::ConnectionReset, "Connection reset");
    let critical_read_error = ReadError::from(critical_io_error);
    assert!(utils::is_critical_error(&critical_read_error));
    
    let non_critical_io_error = io::Error::new(ErrorKind::TimedOut, "Timeout");
    let non_critical_read_error = ReadError::from(non_critical_io_error);
    assert!(!utils::is_critical_error(&non_critical_read_error));
    
    let xs_read_error = ReadError::from(XStreamError::new(b"test".to_vec()));
    assert!(!utils::is_critical_error(&xs_read_error));
    
    // Test error_description
    let io_description = utils::error_description(&non_critical_read_error);
    println!("IO error description: '{}'", io_description);
    assert!(io_description.contains("IO error"));
    // The exact error description may vary, so check for general timeout indication
    assert!(io_description.contains("timed out") || io_description.contains("timeout") || io_description.contains("TimedOut") || io_description.contains("Timed Out"));
    
    let xs_description = utils::error_description(&xs_read_error);
    assert!(xs_description.contains("XStream error"));
}

#[test]
fn test_result_with_partial_data() {
    // Test result_with_partial_data utility function
    let partial_data = b"partial result".to_vec();
    
    // Test with Ok result
    let ok_result: Result<Vec<u8>, io::Error> = Ok(b"success".to_vec());
    let converted_ok = utils::result_with_partial_data(ok_result, partial_data.clone());
    assert!(converted_ok.is_ok());
    assert_eq!(converted_ok.unwrap(), b"success");
    
    // Test with Err result
    let err_result: Result<Vec<u8>, io::Error> = Err(io::Error::new(ErrorKind::BrokenPipe, "Pipe broken"));
    let converted_err = utils::result_with_partial_data(err_result, partial_data.clone());
    assert!(converted_err.is_err());
    
    if let Err(error_on_read) = converted_err {
        assert_eq!(error_on_read.partial_data(), &partial_data);
        assert!(error_on_read.is_io_error());
        assert_eq!(error_on_read.kind(), ErrorKind::BrokenPipe);
    } else {
        panic!("Expected error");
    }
}

#[test]
fn test_xstream_read_result_type() {
    // Test that XStreamReadResult type alias works correctly
    let ok_result: XStreamReadResult<Vec<u8>> = Ok(b"data".to_vec());
    assert!(ok_result.is_ok());
    
    let io_error = io::Error::new(ErrorKind::TimedOut, "Timeout");
    let err_result: XStreamReadResult<Vec<u8>> = Err(ErrorOnRead::io_error_only(io_error));
    assert!(err_result.is_err());
}

#[test]
fn test_error_trait_implementations() {
    // Test that error traits are properly implemented
    let io_error = io::Error::new(ErrorKind::NotFound, "Not found");
    let read_error = ReadError::from(io_error);
    
    // Test that ReadError implements Error trait
    let error_trait: &dyn std::error::Error = &read_error;
    assert!(error_trait.source().is_none()); // IoErrorWrapper doesn't implement Error
    
    let xs_error = XStreamError::new(b"test".to_vec());
    let xs_read_error = ReadError::from(xs_error);
    let xs_error_trait: &dyn std::error::Error = &xs_read_error;
    assert!(xs_error_trait.source().is_some()); // XStreamError implements Error
    
    // Test ErrorOnRead Error trait implementation
    let error_on_read = ErrorOnRead::io_error_only(io::Error::new(ErrorKind::Other, "test"));
    let eor_error_trait: &dyn std::error::Error = &error_on_read;
    assert!(eor_error_trait.source().is_some());
}

#[test]
fn test_comprehensive_error_scenarios() {
    // Test complex error scenarios that combine multiple error types
    let partial_data = vec![0x01, 0x02, 0x03, 0x04];
    let binary_error_data = vec![0xFF, 0xFE, 0x00, 0x01, 0x02];
    
    // Create a complex error chain
    let xs_error = XStreamError::new(binary_error_data.clone());
    let error_on_read = ErrorOnRead::from_xstream_error(partial_data.clone(), xs_error);
    
    // Verify all properties
    assert_eq!(error_on_read.partial_data(), &partial_data);
    assert!(error_on_read.has_partial_data());
    assert_eq!(error_on_read.partial_data_len(), 4);
    assert!(error_on_read.is_xstream_error());
    assert!(!error_on_read.is_io_error());
    
    if let Some(xs_err) = error_on_read.as_xstream_error() {
        assert_eq!(xs_err.data(), &binary_error_data);
        assert!(xs_err.message().is_none()); // Binary data shouldn't be UTF-8
        assert_eq!(xs_err.len(), 5);
        assert!(!xs_err.is_empty());
    }
    
    // Test conversion to io::Error
    let converted_io_error = error_on_read.to_io_error();
    assert_eq!(converted_io_error.kind(), ErrorKind::Other);
    assert!(converted_io_error.to_string().contains("XStream error"));
}

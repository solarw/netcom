# XStream Protocol - Developer Guide

## Overview

**XStream** is an innovative dual-stream protocol built on libp2p that provides reliable data transmission with enhanced error handling. The core innovation is using two separate streams for each logical connection: one for data and one for asynchronous error reporting.

## Key Concepts

### Dual-Stream Architecture
- **Main Stream**: Regular data transmission
- **Error Stream**: Asynchronous error messages
- **15-second timeout**: For stream pairing completion
- **Direction-aware**: Different rules for inbound/outbound streams

### Stream States
- `Open` - Active and operational
- `LocalClosed` - Closed by local peer
- `RemoteClosed` - Closed by remote peer
- `FullyClosed` - Both sides closed
- `Error` - Error occurred

## Component Architecture

### Core Files

#### `xstream.rs` - Main XStream Implementation
- **Purpose**: Main XStream struct with async operations
- **Key Features**:
  - Dual-stream management (main + error)
  - Async read/write operations with error awareness
  - State management and lifecycle
  - Direction-aware behavior

#### `behaviour.rs` - libp2p NetworkBehaviour
- **Purpose**: Integrates XStream with libp2p swarm
- **Key Features**:
  - Handles incoming/outgoing stream requests
  - Manages protocol negotiation
  - Coordinates with PendingStreamsManager

#### `pending_streams.rs` - Stream Pairing Logic
- **Purpose**: Manages pairing of main and error streams
- **Key Features**:
  - 15-second timeout for stream pairing
  - Error handling for same-role streams
  - Message-based communication with behaviour

#### `header.rs` - Stream Headers
- **Purpose**: Serialization/deserialization of stream headers
- **Key Features**:
  - XStreamID (128-bit identifier)
  - SubstreamType (Main/Error)
  - NetworkEndian serialization

#### `types.rs` - Core Types
- **Purpose**: Type definitions and enums
- **Key Features**:
  - `XStreamDirection` (Inbound/Outbound)
  - `SubstreamType` (Main/Error)
  - `XStreamState` enum

#### `xstream_state.rs` - State Management
- **Purpose**: Manages stream state transitions
- **Key Features**:
  - Atomic state updates
  - State transition validation
  - Notification system

## Current Implementation Status

### ✅ Completed Features
- **XStream Core**: Full dual-stream implementation
- **Async Operations**: All read/write operations
- **Error Handling**: Comprehensive error management
- **State Management**: Complete state machine
- **Testing**: Extensive unit and integration tests
- **Documentation**: Detailed technical docs

### 🚧 In Progress
- **PendingStreamsManager**: Stream pairing logic (implementation needed)
- **Edge Cases**: Some timeout scenarios require refinement

### ❌ Known Issues
- **Stream Timeouts**: 15-second timeout handling needs optimization
- **Error Recovery**: Some edge cases in error recovery paths
- **Memory Usage**: Potential optimizations in stream management

## Integration Guide

### Adding to Cargo.toml
```toml
[dependencies]
xstream = { path = "../netcom/protocols/xstream" }
```

### Basic Usage
```rust
use xstream::{XStream, XStreamDirection};

// Create XStream from paired libp2p streams
let xstream = XStream::new(
    stream_id,
    peer_id,
    main_read,
    main_write,
    error_read,
    error_write,
    direction,
    closure_notifier,
);

// Read data with error awareness
let data = xstream.read().await?;

// Write data
xstream.write_all(data).await?;

// Handle errors (inbound streams only)
xstream.error_write(error_data).await?;
```

### Integration with libp2p Behaviour
```rust
use xstream::behaviour::{XStreamBehaviour, XStreamBehaviourEvent};

let mut behaviour = XStreamBehaviour::new();
// Add to your libp2p swarm
```

## Key Implementation Details

### Stream Pairing Process
1. **Outbound Stream Creation**:
   - Generate XStreamID
   - Open main stream with header
   - Open error stream with same XStreamID
   - Wait for both streams within 15 seconds

2. **Inbound Stream Handling**:
   - Read header to get XStreamID and type
   - Store in pending streams
   - Wait for paired stream within 15 seconds
   - Create XStream when both streams received

### Error Handling Strategy
- **Outbound Streams**: Can read errors from error stream
- **Inbound Streams**: Can write errors to error stream
- **Async Error Detection**: Background task monitors error stream
- **Error Prioritization**: Errors interrupt read operations

### State Management
- **Atomic Operations**: Thread-safe state updates
- **Transition Validation**: Prevents invalid state changes
- **Notification System**: Events for state changes
- **Cleanup**: Automatic resource management

## Development Guidelines

### Adding New Features
1. **Extend Types**: Update `types.rs` with new enums/structs
2. **Update XStream**: Add methods to `xstream.rs`
3. **Modify Behaviour**: Update `behaviour.rs` for new events
4. **Add Tests**: Comprehensive test coverage
5. **Update Documentation**: Keep docs synchronized

### Common Patterns
```rust
// Utility methods reduce code duplication
async fn execute_main_read_op<F, R>(&self, operation: F) -> Result<R, std::io::Error>

// State checking before operations
fn check_readable(&self) -> XStreamReadResult<()>
fn check_writable(&self) -> Result<(), std::io::Error>
```

### Testing Strategy
- **Unit Tests**: Individual component testing
- **Integration Tests**: Full protocol testing
- **Edge Cases**: Timeout, error, and failure scenarios
- **Performance**: Stream pairing and data transfer

## Next Development Steps

### Priority 1: Complete PendingStreamsManager
- Implement stream pairing with 15s timeout
- Handle same-role stream errors
- Integrate with XStream behaviour
- Add comprehensive tests

### Priority 2: Optimize Performance
- Reduce memory usage in stream management
- Optimize timeout handling
- Improve error recovery paths
- Benchmark and profile critical paths

### Priority 3: Enhance Reliability
- Add more edge case handling
- Improve connection error recovery
- Add metrics and monitoring
- Stress testing under load

## For AI Assistants

### Key Files to Modify
- `pending_streams.rs` - Highest priority for completion
- `behaviour.rs` - Integration points
- `xstream.rs` - Core functionality
- `types.rs` - Type definitions

### Common Extension Points
- New stream operations in `xstream.rs`
- Additional events in `behaviour.rs`
- New state transitions in `xstream_state.rs`
- Enhanced error types in error handling modules

### Testing Changes
```bash
# Run XStream tests
cargo test -p xstream

# Run specific test modules
cargo test -p xstream test_stream_pairing
cargo test -p xstream test_error_handling
```

### Integration Testing
- Test with xnetwork library
- Verify Python bindings work
- Check compatibility with existing tests

---

*This document focuses exclusively on the XStream protocol component. Last updated: October 2025*

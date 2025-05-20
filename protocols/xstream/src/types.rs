use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Direction of the XStream
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XStreamDirection {
    /// Inbound stream (initiated by remote peer)
    Inbound,
    /// Outbound stream (initiated by local node)
    Outbound,
}

/// Role of the substream within XStream
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstreamRole {
    /// Main data stream
    Main = 0,
    /// Error reporting stream
    Error = 1,
}

impl From<u8> for SubstreamRole {
    fn from(value: u8) -> Self {
        match value {
            1 => SubstreamRole::Error,
            _ => SubstreamRole::Main,
        }
    }
}
/// Stream states for XStream
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum XStreamState {
    /// Stream is open in both directions
    Open = 0,
    /// Stream's write direction is locally closed (EOF sent)
    WriteLocalClosed = 1,
    /// Stream's read direction has received EOF from remote
    ReadRemoteClosed = 2,
    /// Stream is locally closed (both read and write)
    LocalClosed = 3,
    /// Stream is remotely closed (both read and write)
    RemoteClosed = 4,
    /// Stream is fully closed in both directions
    FullyClosed = 5,
    /// Stream has encountered an error
    Error = 6,
}

impl From<u8> for XStreamState {
    fn from(value: u8) -> Self {
        match value {
            1 => XStreamState::WriteLocalClosed,
            2 => XStreamState::ReadRemoteClosed,
            3 => XStreamState::LocalClosed,
            4 => XStreamState::RemoteClosed,
            5 => XStreamState::FullyClosed,
            6 => XStreamState::Error,
            _ => XStreamState::Open,
        }
    }
}

/// Unique identifier for XStream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XStreamID(pub u128);

impl From<u128> for XStreamID {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl From<XStreamID> for u128 {
    fn from(id: XStreamID) -> Self {
        id.0
    }
}

// Реализация Display для XStreamID
impl std::fmt::Display for XStreamID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Iterator for generating unique XStreamID values
#[derive(Debug)]
pub struct XStreamIDIterator {
    /// Current high bits of the ID
    high: Arc<AtomicU64>,
    /// Current low bits of the ID
    low: Arc<AtomicU64>,
}

impl XStreamIDIterator {
    /// Creates a new XStreamIDIterator starting from 0
    pub fn new() -> Self {
        Self {
            high: Arc::new(AtomicU64::new(0)),
            low: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Creates a new XStreamIDIterator starting from a specific value
    pub fn with_start(start: u128) -> Self {
        let high = (start >> 64) as u64;
        let low = start as u64;

        Self {
            high: Arc::new(AtomicU64::new(high)),
            low: Arc::new(AtomicU64::new(low)),
        }
    }
}

impl Iterator for XStreamIDIterator {
    type Item = XStreamID;

    fn next(&mut self) -> Option<Self::Item> {
        // Increment the low part first
        let low = self.low.fetch_add(1, Ordering::SeqCst);

        // If low part wrapped around to 0, increment the high part
        if low == u64::MAX {
            self.high.fetch_add(1, Ordering::SeqCst);
        }

        // Combine high and low parts into u128
        let high_u128 = (self.high.load(Ordering::SeqCst) as u128) << 64;
        let current = high_u128 | (low as u128);

        // Return the current value as XStreamID
        Some(XStreamID(current))
    }
}

impl Clone for XStreamIDIterator {
    fn clone(&self) -> Self {
        Self {
            high: self.high.clone(),
            low: self.low.clone(),
        }
    }
}

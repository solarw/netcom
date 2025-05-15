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

/// Stream closure states
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum XStreamState {
    /// Stream is open in both directions
    Open = 0,
    /// Stream is locally closed (can still receive data)
    LocalClosed = 1,
    /// Stream is remotely closed (can still send data)
    RemoteClosed = 2,
    /// Stream is fully closed in both directions
    FullyClosed = 3,
}

impl From<u8> for XStreamState {
    fn from(value: u8) -> Self {
        match value {
            1 => XStreamState::LocalClosed,
            2 => XStreamState::RemoteClosed,
            3 => XStreamState::FullyClosed,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xstream_id_iterator() {
        let mut iter = XStreamIDIterator::new();
        
        assert_eq!(iter.next(), Some(XStreamID(0)));
        assert_eq!(iter.next(), Some(XStreamID(1)));
        assert_eq!(iter.next(), Some(XStreamID(2)));
        
        // Test that clone continues from the same sequence
        let mut clone_iter = iter.clone();
        assert_eq!(clone_iter.next(), Some(XStreamID(3)));
        assert_eq!(iter.next(), Some(XStreamID(4)));
    }
    
    #[test]
    fn test_xstream_id_conversion() {
        let id = XStreamID(42);
        let value: u128 = id.into();
        assert_eq!(value, 42);
        
        let id_back = XStreamID::from(value);
        assert_eq!(id, id_back);
    }
    
    #[test]
    fn test_xstream_id_display() {
        let id = XStreamID(42);
        assert_eq!(format!("{}", id), "42");
    }
    
    #[test]
    fn test_substream_role_from_u8() {
        assert_eq!(SubstreamRole::from(0), SubstreamRole::Main);
        assert_eq!(SubstreamRole::from(1), SubstreamRole::Error);
        assert_eq!(SubstreamRole::from(2), SubstreamRole::Main); // Default to Main for unknown values
    }
    
    #[test]
    fn test_xstream_state_from_u8() {
        assert_eq!(XStreamState::from(0), XStreamState::Open);
        assert_eq!(XStreamState::from(1), XStreamState::LocalClosed);
        assert_eq!(XStreamState::from(2), XStreamState::RemoteClosed);
        assert_eq!(XStreamState::from(3), XStreamState::FullyClosed);
        assert_eq!(XStreamState::from(4), XStreamState::Open); // Default to Open for unknown values
    }
}
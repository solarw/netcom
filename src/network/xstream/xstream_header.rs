// src/network/xstream/xstream_header.rs
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::Stream;
use std::io::{self, Cursor};

use super::types::{XStreamID, SubstreamRole};

/// Header for stream identification
#[derive(Debug, Clone)]
pub struct XStreamHeader {
    pub stream_id: XStreamID,
    pub stream_type: SubstreamRole,
}

impl XStreamHeader {
    /// Create a new XStreamHeader
    pub fn new(stream_id: XStreamID, stream_type: SubstreamRole) -> Self {
        Self {
            stream_id,
            stream_type,
        }
    }

    /// Write a stream header (stream_id and stream_type)
    pub async fn write_header(
        stream: &mut futures::io::WriteHalf<Stream>, 
        header: &XStreamHeader
    ) -> Result<(), io::Error> {
        // Create a buffer for the header
        let mut header_buf = Vec::new();
        
        // Write stream ID (u128) in network byte order
        header_buf.write_u128::<NetworkEndian>(header.stream_id.into())?;
        
        // Write stream type (1 byte)
        header_buf.write_u8(header.stream_type as u8)?;
        
        // Write the header to the stream
        stream.write_all(&header_buf).await?;
        stream.flush().await?;
        
        Ok(())
    }
    
    /// Read a stream header
    pub async fn read_header(
        stream: &mut futures::io::ReadHalf<Stream>
    ) -> Result<XStreamHeader, io::Error> {
        // Read stream ID (16 bytes)
        let mut id_buf = vec![0u8; 16];
        stream.read_exact(&mut id_buf).await?;
        
        let mut cursor = Cursor::new(id_buf);
        let stream_id_raw = cursor.read_u128::<NetworkEndian>()?;
        let stream_id = XStreamID(stream_id_raw);
        
        // Read stream type (1 byte)
        let mut type_buf = [0u8; 1];
        stream.read_exact(&mut type_buf).await?;
        
        let stream_type = SubstreamRole::from(type_buf[0]);
        
        Ok(XStreamHeader {
            stream_id,
            stream_type,
        })
    }
}
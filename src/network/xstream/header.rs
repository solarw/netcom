use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::Stream;
use std::io::{self, Cursor};

use super::types::{SubstreamRole, XStreamID};

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
}

/// Write a stream header (stream_id and stream_type)
pub async fn write_header<W>(writer: &mut W, header: &XStreamHeader) -> Result<(), io::Error>
where
    W: AsyncWriteExt + Unpin,
{
    // Create a buffer for the header
    let mut header_buf = Vec::new();

    // Write stream ID (u128) in network byte order
    header_buf.write_u128::<NetworkEndian>(header.stream_id.into())?;

    // Write stream type (1 byte)
    header_buf.write_u8(header.stream_type as u8)?;

    // Write the header to the stream
    writer.write_all(&header_buf).await?;
    writer.flush().await?;

    Ok(())
}

/// Read a stream header
pub async fn read_header<R>(reader: &mut R) -> Result<XStreamHeader, io::Error>
where
    R: AsyncReadExt + Unpin,
{
    // Read stream ID (16 bytes)
    let mut id_buf = vec![0u8; 16];
    reader.read_exact(&mut id_buf).await?;

    let mut cursor = Cursor::new(id_buf);
    let stream_id_raw = cursor.read_u128::<NetworkEndian>()?;
    let stream_id = XStreamID(stream_id_raw);

    // Read stream type (1 byte)
    let mut type_buf = [0u8; 1];
    reader.read_exact(&mut type_buf).await?;

    let stream_type = SubstreamRole::from(type_buf[0]);

    Ok(XStreamHeader {
        stream_id,
        stream_type,
    })
}

/// Write a stream header directly to a Stream
pub async fn write_header_to_stream(
    stream: &mut futures::io::WriteHalf<Stream>,
    header: &XStreamHeader,
) -> Result<(), io::Error> {
    write_header(stream, header).await
}

/// Read a stream header directly from a Stream
pub async fn read_header_from_stream(
    stream: &mut futures::io::ReadHalf<Stream>,
) -> Result<XStreamHeader, io::Error> {
    read_header(stream).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::io::Cursor as AsyncCursor;
    use futures::AsyncWrite;

    #[tokio::test]
    async fn test_header_write_read() {
        // Create a test header
        let stream_id = XStreamID(123456789);
        let stream_type = SubstreamRole::Main;
        let header = XStreamHeader::new(stream_id, stream_type);

        // Buffer to write to
        let mut buffer = Vec::new();
        let mut writer = AsyncCursor::new(&mut buffer);

        // Write header
        write_header(&mut writer, &header).await.unwrap();

        // Read header back
        let mut reader = AsyncCursor::new(&buffer);
        let read_header = read_header(&mut reader).await.unwrap();

        // Verify values
        assert_eq!(read_header.stream_id.0, stream_id.0);
        assert_eq!(read_header.stream_type, stream_type);
    }

    #[tokio::test]
    async fn test_header_different_roles() {
        // Create headers with different roles
        let stream_id = XStreamID(42);
        let main_header = XStreamHeader::new(stream_id, SubstreamRole::Main);
        let error_header = XStreamHeader::new(stream_id, SubstreamRole::Error);

        // Buffer to write to
        let mut main_buffer = Vec::new();
        let mut error_buffer = Vec::new();

        let mut main_writer = AsyncCursor::new(&mut main_buffer);
        let mut error_writer = AsyncCursor::new(&mut error_buffer);

        // Write headers
        write_header(&mut main_writer, &main_header).await.unwrap();
        write_header(&mut error_writer, &error_header)
            .await
            .unwrap();

        // Read headers back
        let mut main_reader = AsyncCursor::new(&main_buffer);
        let mut error_reader = AsyncCursor::new(&error_buffer);

        let read_main = read_header(&mut main_reader).await.unwrap();
        let read_error = read_header(&mut error_reader).await.unwrap();

        // Verify values
        assert_eq!(read_main.stream_type, SubstreamRole::Main);
        assert_eq!(read_error.stream_type, SubstreamRole::Error);
    }
}

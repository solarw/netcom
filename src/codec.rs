use crate::protocol::{EchoRequest, EchoResponse};
use futures::prelude::*;
use libp2p::{request_response::Codec, StreamProtocol};
use std::io;
use async_trait::async_trait;

// Определяем кодек для эхо-протокола
#[derive(Default, Clone)]
pub struct EchoCodec;

#[async_trait]
impl Codec for EchoCodec {
    type Protocol = StreamProtocol;
    type Request = EchoRequest;
    type Response = EchoResponse;

    // Точное соответствие сигнатуре трейта
    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut len_bytes = [0u8; 2];
        io.read_exact(&mut len_bytes).await?;
        let len = u16::from_be_bytes(len_bytes) as usize;
        
        let mut buf = vec![0u8; len];
        io.read_exact(&mut buf).await?;
        
        Ok(EchoRequest(buf))
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut len_bytes = [0u8; 2];
        io.read_exact(&mut len_bytes).await?;
        let len = u16::from_be_bytes(len_bytes) as usize;
        
        let mut buf = vec![0u8; len];
        io.read_exact(&mut buf).await?;
        
        Ok(EchoResponse(buf))
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        EchoRequest(data): Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let len = data.len();
        if len > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "message too long",
            ));
        }
        io.write_all(&(len as u16).to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        EchoResponse(data): Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let len = data.len();
        if len > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "message too long",
            ));
        }
        io.write_all(&(len as u16).to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
    }
}
use core::net::SocketAddr;
use core::time::Duration;

use embedded_io::ErrorType;
use embedded_io_async::{Read, Write};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::TransportError;
use crate::transport::MqttTransport;

/// A transport implementation for MQTT over a Tokio-based TCP stream.
pub struct TcpTransport {
    stream: Option<TcpStream>,
}

impl ErrorType for TcpTransport {
    type Error = TransportError;
}

impl TcpTransport {
    /// Creates a new `TcpTransport` and connects to the given address.
    pub async fn new(addr: SocketAddr) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr)
            .await
            .map_err(TransportError::from)?;
        Ok(Self {
            stream: Some(stream),
        })
    }

    /// Creates a `TcpTransport` with a connection timeout.
    pub async fn new_with_timeout(
        addr: SocketAddr,
        connect_timeout: Duration,
    ) -> Result<Self, TransportError> {
        let connect_future = TcpStream::connect(addr);
        let stream = tokio::time::timeout(connect_timeout, connect_future)
            .await
            .map_err(|_| TransportError::Timeout)? // Outer error is from timeout
            .map_err(TransportError::from)?; // Inner error is from connect

        Ok(Self {
            stream: Some(stream),
        })
    }

    fn get_stream_mut(&mut self) -> Result<&mut TcpStream, TransportError> {
        self.stream.as_mut().ok_or(TransportError::ConnectionLost)
    }
}

impl MqttTransport for TcpTransport {
    async fn close(&mut self) -> Result<(), TransportError> {
        if let Some(mut stream) = self.stream.take() {
            stream.shutdown().await.map_err(TransportError::from)?;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

impl Read for TcpTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let stream = self.get_stream_mut()?;
        stream.read(buf).await.map_err(TransportError::from)
    }
}

impl Write for TcpTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let stream = self.get_stream_mut()?;
        stream.write(buf).await.map_err(TransportError::from)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        let stream = self.get_stream_mut()?;
        stream.flush().await.map_err(TransportError::from)
    }
}
